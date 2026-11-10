#![deny(unused_must_use)]
#![cfg_attr(not(debug_assertions), deny(clippy::dbg_macro))]

mod attribute;
mod entry;
mod lock;
mod utils;

pub(crate) mod err;
pub use entry::PyMftEntry;
use mft_rs::csv::FlatMftEntryWithName;
use mft_rs::entry::ZERO_HEADER;
use mft_rs::{MftEntry, MftParser};

use std::fs::File;
use std::io;
use std::io::{BufReader, Read, Seek};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use pyo3::exceptions;
use pyo3::prelude::*;

use crate::attribute::{
    PyMftAce, PyMftAcl, PyMftAttribute, PyMftAttributeNonResident, PyMftAttributeOther,
    PyMftAttributeX10, PyMftAttributeX20, PyMftAttributeX20Entry, PyMftAttributeX30,
    PyMftAttributeX40, PyMftAttributeX50, PyMftAttributeX60, PyMftAttributeX70, PyMftAttributeX80,
    PyMftAttributeX90, PyMftAttributeXC0, PyMftAttributeXD0, PyMftAttributeXE0, PyMftDataRun,
    PyMftEaEntry, PyMftIndexEntry, PyMftViewIndexEntry, PyMftX20EntriesIter,
};
use crate::entry::PyMftAttributesIter;
use crate::err::{PyMftError, skip_or_raise};
use crate::lock::{ReentryCheckedGuard, ReentryCheckedMutex};
use crate::utils::{FileOrFileLike, deferring_warnings, init_logging, with_deferred_warnings};
use csv::WriterBuilder;
use pyo3::types::{PyBytes, PyString};

pub trait ReadSeek: Read + Seek {
    fn tell(&mut self) -> io::Result<u64> {
        self.stream_position()
    }
}

impl<T: Read + Seek> ReadSeek for T {}

type BoxedParser = MftParser<Box<dyn ReadSeek + Send + Sync>>;
type SharedParser = Arc<ReentryCheckedMutex<BoxedParser>>;

/// Locks the parser shared by a `PyMftParser` and its iterators.
///
/// The lock is held while Python code runs (a file-like object's `read` and `seek`). If that
/// code calls back into the same parser, it gets a `RuntimeError`. Warnings are sent once it is
/// released (`with_deferred_warnings`), so a `warnings` hook can use the parser.
fn lock_parser<'a>(
    parser: &'a SharedParser,
    py: Python<'_>,
) -> PyResult<ReentryCheckedGuard<'a, BoxedParser>> {
    parser.lock(py, "the parser")
}

pub enum Output {
    Python,
    CSV,
    JSON,
}

#[pyclass(frozen, module = "mft")]
/// PyMftParser(self, path_or_file_like, *, skip_errors=False)
/// --
///
/// Returns an instance of the parser.
/// Works on both a path (`str`, `bytes` or `os.PathLike`, such as `pathlib.Path`), or a file-like
/// object opened in binary mode. An object with `read` and `seek` is read as a file object, also
/// when it is an `os.PathLike`.
///
/// A parser can be iterated any number of times, and its entries read by number with
/// `get_entry`, including while an iteration is in progress.
///
/// A file-like object is read at the parser's own position: the parser seeks before every read of
/// the object (entries read in order are fetched 64 KiB at a time, so up to that much ahead of
/// the entry asked for). So the object can be used elsewhere between the parser's calls, by
/// another parser too. That
/// only holds on one thread. A seek and its read are two calls on the object, so if another
/// thread uses the object at the same time (directly, or through a second parser over it), the
/// two can interleave and a read can return the bytes of another record. Threads can share one
/// parser, whose lock covers both calls; give each thread's parser its own file object.
///
/// Iterating over entries or attributes raises `RuntimeError` or `OSError` for one that fails to
/// parse, after stepping past it, so `next()` can go on. With `skip_errors=True` it is skipped
/// instead, with a `RuntimeWarning` that `warnings` filters can count, log or turn back into an
/// error. Failing to read the file raises either way.
///
/// What the parser notices but reads anyway, such as a record whose fixup values don't match, is
/// reported as a `UserWarning`. If a `warnings` filter turns it into an error (`-W error`), the
/// call that read the record raises it and doesn't return the entry: `get_entry` raises every
/// time, and an iterator steps past the entry, as for one that fails to parse.
pub struct PyMftParser {
    inner: SharedParser,
    /// Fixed once the parser is built, so reading it doesn't need the lock.
    entry_count: u64,
    skip_errors: bool,
}

#[pymethods]
impl PyMftParser {
    #[new]
    #[pyo3(signature = (path_or_file_like, *, skip_errors = false))]
    fn new(path_or_file_like: &Bound<'_, PyAny>, skip_errors: bool) -> PyResult<Self> {
        let file_or_file_like = FileOrFileLike::from_pyobject(path_or_file_like)?;

        let (boxed_read_seek, size) = match file_or_file_like {
            FileOrFileLike::File(s) => {
                let file = File::open(s)?;
                let size = file.metadata()?.len();

                let reader = BufReader::with_capacity(4096, file);

                (
                    Box::new(reader) as Box<dyn ReadSeek + Send + Sync>,
                    Some(size),
                )
            }
            FileOrFileLike::FileLike(f) => (Box::new(f) as Box<dyn ReadSeek + Send + Sync>, None),
        };

        let parser = MftParser::from_read_seek(boxed_read_seek, size).map_err(PyMftError)?;

        Ok(PyMftParser {
            entry_count: parser.get_entry_count(),
            inner: Arc::new(ReentryCheckedMutex::new(parser)),
            skip_errors,
        })
    }

    /// number_of_entries(self, /)
    /// --
    ///
    /// Returns the total number of entries in the MFT.
    fn number_of_entries(&self) -> u64 {
        self.entry_count
    }

    /// get_entry(self, entry_id)
    /// --
    ///
    /// Returns the entry with this number, or `None` if its record is all zeroes (an entry
    /// `entries()` skips). Raises `IndexError` for a negative number or one past the last entry.
    fn get_entry(
        &self,
        py: Python,
        entry_id: &Bound<'_, PyAny>,
    ) -> PyResult<Option<Py<PyMftEntry>>> {
        let count = self.entry_count;
        let entry_id = match entry_id.extract::<u64>() {
            Ok(id) if id < count => id,
            // An integer, but negative or over 64 bits.
            Err(error) if !error.is_instance_of::<exceptions::PyOverflowError>(py) => {
                return Err(error);
            }
            _ => {
                return Err(PyErr::new::<exceptions::PyIndexError, _>(format!(
                    "entry {entry_id} is out of range ({count} entries)"
                )));
            }
        };
        with_deferred_warnings(py, || {
            let mut parser = lock_parser(&self.inner, py)?;
            let entry = parser.get_entry(entry_id).map_err(PyMftError)?;
            if &entry.header.signature == ZERO_HEADER {
                return Ok(None);
            }
            PyMftEntry::from_mft_entry(py, entry, &mut parser, self.skip_errors).map(Some)
        })
    }

    /// entries(self, /)
    /// --
    ///
    /// Returns an iterator that yields the mft entries as python objects.
    fn entries(&self, py: Python) -> PyResult<Py<PyMftEntriesIterator>> {
        self.records_iterator(py, Output::Python)
    }

    /// entries_json(self, /)
    /// --
    ///
    /// Returns an iterator that yields mft entries as JSON.
    fn entries_json(&self, py: Python) -> PyResult<Py<PyMftEntriesIterator>> {
        self.records_iterator(py, Output::JSON)
    }

    /// entries_csv(self, /)
    /// --
    ///
    /// Returns an iterator that yields mft entries CSV lines.
    fn entries_csv(&self, py: Python) -> PyResult<Py<PyMftEntriesIterator>> {
        self.records_iterator(py, Output::CSV)
    }

    fn __iter__(slf: PyRef<Self>) -> PyResult<Py<PyMftEntriesIterator>> {
        slf.entries(slf.py())
    }

    fn __repr__(&self) -> String {
        format!(
            "<PyMftParser number_of_entries={} skip_errors={}>",
            self.entry_count,
            if self.skip_errors { "True" } else { "False" }
        )
    }
}

impl PyMftParser {
    fn records_iterator(
        &self,
        py: Python,
        output_format: Output,
    ) -> PyResult<Py<PyMftEntriesIterator>> {
        Py::new(
            py,
            PyMftEntriesIterator {
                total_number_of_records: self.entry_count,
                inner: Arc::clone(&self.inner),
                output_format,
                skip_errors: self.skip_errors,
                next_record: Mutex::new(0),
                csv_header_written: AtomicBool::new(false),
            },
        )
    }
}

/// Threads can share an iterator: each entry goes to one of them.
#[pyclass(frozen, module = "mft")]
pub struct PyMftEntriesIterator {
    inner: SharedParser,
    total_number_of_records: u64,
    output_format: Output,
    skip_errors: bool,
    /// The number of the next record to read. Only locked with the parser locked, so it is never
    /// waited for, and a thread that calls the iterator again from its file object's `read` is
    /// stopped at the parser's lock. Held as long as the parser, so entries leave in order.
    next_record: Mutex<u64>,
    /// Whether a row was returned with the CSV header.
    csv_header_written: AtomicBool,
}

#[pymethods]
impl PyMftEntriesIterator {
    fn __iter__(slf: PyRef<Self>) -> Py<PyMftEntriesIterator> {
        slf.into()
    }

    fn __next__(&self, py: Python) -> PyResult<Option<Py<PyAny>>> {
        loop {
            // Unlocked while warning, as a `warnings` hook may use the parser or this iterator.
            let (step, warned) = deferring_warnings(py, || self.next_entry(py));
            let step = step?;
            // The entry that warned isn't returned.
            warned?;
            match step {
                Step::End => return Ok(None),
                Step::Yield(item) => return Ok(Some(item)),
                Step::YieldCsv { row, header_length } => {
                    // The header goes with the first row that is returned, which the first one
                    // read may not be.
                    let had_header =
                        header_length > 0 && self.csv_header_written.swap(true, Ordering::Relaxed);
                    let row = if had_header {
                        &row[header_length..]
                    } else {
                        &row[..]
                    };
                    return Ok(Some(PyBytes::new(py, row).into_any().unbind()));
                }
                Step::Failed(id, error) => {
                    skip_or_raise(py, self.skip_errors, format_args!("entry {id}"), error)?
                }
            }
        }
    }
}

/// What reading the next entry came to, once the locks are released.
enum Step {
    /// There are no more entries.
    End,
    /// The entry, as an object or JSON.
    Yield(Py<PyAny>),
    /// The entry's CSV row, after a header of this length if none was returned when it was read.
    YieldCsv { row: Vec<u8>, header_length: usize },
    /// The entry with this number failed to parse.
    Failed(u64, mft_rs::err::Error),
}

impl PyMftEntriesIterator {
    fn next_entry(&self, py: Python) -> PyResult<Step> {
        let mut parser = lock_parser(&self.inner, py)?;
        let mut next_record = self
            .next_record
            .lock()
            .unwrap_or_else(PoisonError::into_inner);

        while *next_record < self.total_number_of_records {
            let id = *next_record;
            *next_record += 1;

            let entry = match parser.get_entry(id) {
                Ok(entry) if &entry.header.signature == ZERO_HEADER => continue,
                Ok(entry) => entry,
                // The file failed to read, which skipping the entry doesn't fix.
                Err(error @ mft_rs::err::Error::IoError { .. }) => {
                    return Err(PyMftError(error).into());
                }
                Err(error) => return Ok(Step::Failed(id, error)),
            };
            return match self.output_format {
                Output::Python => {
                    PyMftEntry::from_mft_entry(py, entry, &mut parser, self.skip_errors)
                        .map(|entry| Step::Yield(entry.into_any()))
                }
                Output::JSON => Self::entry_to_json(entry, py).map(Step::Yield),
                Output::CSV => {
                    let with_header = !self.csv_header_written.load(Ordering::Relaxed);
                    Self::entry_to_csv(with_header, &mut parser, entry)
                }
            };
        }
        Ok(Step::End)
    }

    fn entry_to_json(entry: MftEntry, py: Python) -> PyResult<Py<PyAny>> {
        let json = sonic_rs::to_string(&entry).map_err(|_| {
            PyErr::new::<exceptions::PyRuntimeError, _>("JSON Serialization failed")
        })?;
        Ok(PyString::new(py, &json).into_any().unbind())
    }

    fn entry_to_csv(
        with_header: bool,
        parser: &mut BoxedParser,
        entry: MftEntry,
    ) -> PyResult<Step> {
        let mut writer = WriterBuilder::new()
            .has_headers(with_header)
            .from_writer(Vec::new());

        writer
            .serialize(FlatMftEntryWithName::from_entry(&entry, parser))
            .map_err(|_| PyErr::new::<exceptions::PyRuntimeError, _>("CSV Serialization failed"))?;
        let row = writer
            .into_inner()
            .map_err(|e| PyErr::new::<exceptions::PyRuntimeError, _>(e.to_string()))?;
        // The header is the first line: column names have no line breaks.
        let header_length = match row.iter().position(|&byte| byte == b'\n') {
            Some(end) if with_header => end + 1,
            _ => 0,
        };
        Ok(Step::YieldCsv { row, header_length })
    }
}

// Don't use double quotes ("") inside this docstring, this will crash pyo3.
/// Parses an mft file.
#[pymodule(gil_used = false)]
fn mft(m: &Bound<'_, PyModule>) -> PyResult<()> {
    init_logging().ok();

    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    m.add_class::<PyMftParser>()?;

    // Entry
    m.add_class::<PyMftEntriesIterator>()?;
    m.add_class::<PyMftEntry>()?;

    // Attributes
    m.add_class::<PyMftAttribute>()?;
    m.add_class::<PyMftAttributesIter>()?;
    m.add_class::<PyMftAttributeX10>()?;
    m.add_class::<PyMftAttributeX20>()?;
    m.add_class::<PyMftAttributeX20Entry>()?;
    m.add_class::<PyMftX20EntriesIter>()?;
    m.add_class::<PyMftAttributeX30>()?;
    m.add_class::<PyMftAttributeX40>()?;
    m.add_class::<PyMftAttributeX50>()?;
    m.add_class::<PyMftAcl>()?;
    m.add_class::<PyMftAce>()?;
    m.add_class::<PyMftAttributeX60>()?;
    m.add_class::<PyMftAttributeX70>()?;
    m.add_class::<PyMftAttributeX80>()?;
    m.add_class::<PyMftAttributeX90>()?;
    m.add_class::<PyMftAttributeXC0>()?;
    m.add_class::<PyMftIndexEntry>()?;
    m.add_class::<PyMftViewIndexEntry>()?;
    m.add_class::<PyMftAttributeXD0>()?;
    m.add_class::<PyMftAttributeXE0>()?;
    m.add_class::<PyMftEaEntry>()?;
    m.add_class::<PyMftAttributeNonResident>()?;
    m.add_class::<PyMftDataRun>()?;
    m.add_class::<PyMftAttributeOther>()?;

    Ok(())
}
