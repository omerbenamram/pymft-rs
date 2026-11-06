#![deny(unused_must_use)]
#![cfg_attr(not(debug_assertions), deny(clippy::dbg_macro))]

mod attribute;
mod entry;
mod utils;

pub(crate) mod err;
pub use entry::PyMftEntry;
use mft_rs::attribute::MftAttributeType;
use mft_rs::csv::FlatMftEntryWithName;
use mft_rs::entry::ZERO_HEADER;
use mft_rs::{MftEntry, MftParser};

use std::fs::File;
use std::io;
use std::io::{BufReader, Read, Seek};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use pyo3::exceptions;
use pyo3::prelude::*;
use pyo3::sync::MutexExt;

use crate::attribute::{
    PyMftAce, PyMftAcl, PyMftAttribute, PyMftAttributeNonResident, PyMftAttributeOther,
    PyMftAttributeX10, PyMftAttributeX20, PyMftAttributeX20Entry, PyMftAttributeX30,
    PyMftAttributeX40, PyMftAttributeX50, PyMftAttributeX60, PyMftAttributeX70, PyMftAttributeX80,
    PyMftAttributeX90, PyMftAttributeXC0, PyMftAttributeXD0, PyMftAttributeXE0, PyMftDataRun,
    PyMftEaEntry, PyMftIndexEntry, PyMftViewIndexEntry, PyMftX20EntriesIter,
};
use crate::entry::PyMftAttributesIter;
use crate::err::{PyMftError, skip_or_raise};
use crate::utils::{FileOrFileLike, init_logging};
use csv::WriterBuilder;
use pyo3::types::{PyBytes, PyString};

pub trait ReadSeek: Read + Seek {
    fn tell(&mut self) -> io::Result<u64> {
        self.stream_position()
    }
}

impl<T: Read + Seek> ReadSeek for T {}

type BoxedParser = MftParser<Box<dyn ReadSeek + Send + Sync>>;
type SharedParser = Arc<Mutex<BoxedParser>>;

/// Locks the parser shared by a `PyMftParser` and its iterators.
///
/// The GIL is released while waiting, so a thread holding the lock can call back into Python (a
/// file-like object's `read`) without deadlocking. The lock is not reentrant: Python code run
/// while it is held (a file-like object's `read`, a `warnings` hook, a signal handler) must not
/// call back into the same parser. A poisoned lock is recovered, since a panic can only happen
/// after an entry's bytes were read.
fn lock_parser<'a>(parser: &'a SharedParser, py: Python<'_>) -> MutexGuard<'a, BoxedParser> {
    parser
        .lock_py_attached(py)
        .unwrap_or_else(PoisonError::into_inner)
}

pub enum Output {
    Python,
    CSV,
    JSON,
    Bodyfile,
}

#[pyclass(frozen, module = "mft")]
/// PyMftParser(self, path_or_file_like, *, skip_errors=False)
/// --
///
/// Returns an instance of the parser.
/// Works on both a path (string), or a file-like object.
///
/// A parser can be iterated any number of times, and its entries read by number with
/// `get_entry`, including while an iteration is in progress.
///
/// Iterating over entries or attributes raises `RuntimeError` or `OSError` for one that fails to
/// parse, after stepping past it, so `next()` can go on. With `skip_errors=True` it is skipped
/// instead, with a `RuntimeWarning` that `warnings` filters can count, log or turn back into an
/// error. Failing to read the file raises either way.
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
    fn new(path_or_file_like: Py<PyAny>, skip_errors: bool) -> PyResult<Self> {
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
            inner: Arc::new(Mutex::new(parser)),
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
    fn get_entry(&self, py: Python, entry_id: i64) -> PyResult<Option<Py<PyMftEntry>>> {
        let entry_id = self.check_entry_id(entry_id)?;
        let mut parser = lock_parser(&self.inner, py);
        let entry = parser.get_entry(entry_id).map_err(PyMftError)?;
        if &entry.header.signature == ZERO_HEADER {
            return Ok(None);
        }
        PyMftEntry::from_mft_entry(py, entry, &mut parser, self.skip_errors).map(Some)
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

    /// entries_bodyfile(self, /)
    /// --
    ///
    /// Returns an iterator that yields Sleuth Kit bodyfile lines (as `fls -m` writes them, for
    /// `mactime`), one `str` per file holding all of its lines. Extension records and entries
    /// without a `$FILE_NAME` are skipped.
    fn entries_bodyfile(&self, py: Python) -> PyResult<Py<PyMftEntriesIterator>> {
        self.records_iterator(py, Output::Bodyfile)
    }

    /// data_streams(self, entry_id)
    /// --
    ///
    /// Returns the `$DATA` streams of the file with this entry number, including the ones
    /// stored in its extension records: the unnamed stream first, then the named ones
    /// (alternate data streams), in the order they are stored. A resident stream carries its
    /// content in `data`. A non-resident one has `data=None`, since its content is on the volume
    /// rather than in the MFT. An all-zero record has none. Raises `IndexError` like `get_entry`.
    fn data_streams(&self, py: Python, entry_id: i64) -> PyResult<Vec<Py<PyMftDataStream>>> {
        let entry_id = self.check_entry_id(entry_id)?;
        let mut parser = lock_parser(&self.inner, py);
        let entry = parser.get_entry(entry_id).map_err(PyMftError)?;
        // An all-zero record, which `get_entry` returns as `None`.
        if &entry.header.signature == ZERO_HEADER {
            return Ok(Vec::new());
        }
        let extensions = parser.get_extension_entries(&entry);
        drop(parser);

        let mut streams = Vec::new();
        for record in std::iter::once(&entry).chain(&extensions) {
            for attribute in record.iter_attributes_matching(Some(vec![MftAttributeType::DATA])) {
                let attribute = match attribute {
                    Ok(attribute) => attribute,
                    Err(error) => {
                        skip_or_raise(
                            py,
                            self.skip_errors,
                            format_args!(
                                "a $DATA attribute of entry {}",
                                record.header.record_number
                            ),
                            error,
                        )?;
                        continue;
                    }
                };
                // The later pieces of a fragmented non-resident stream.
                if attribute.header.is_continuation() {
                    continue;
                }
                streams.push(Py::new(
                    py,
                    PyMftDataStream {
                        name: attribute.header.name.to_string(),
                        size: attribute.header.value_size(),
                        record_entry_id: record.header.record_number,
                        attribute_id: attribute.header.instance,
                        data: attribute
                            .data
                            .as_data()
                            .map(|data| PyBytes::new(py, data.data()).unbind()),
                    },
                )?);
            }
        }
        // The unnamed stream first.
        streams.sort_by_key(|stream| !stream.get().name.is_empty());
        Ok(streams)
    }

    fn __iter__(slf: PyRef<Self>) -> PyResult<Py<PyMftEntriesIterator>> {
        slf.entries(slf.py())
    }
    fn __next__(_slf: PyRef<Self>) -> PyResult<Option<Py<PyAny>>> {
        Err(PyErr::new::<exceptions::PyNotImplementedError, _>(
            "Using `next()` over `PyMftParser` is not supported. Try iterating over `PyMftParser(...).entries()`",
        ))
    }
}

impl PyMftParser {
    /// Raises `IndexError` for a negative number or one past the last entry.
    fn check_entry_id(&self, entry_id: i64) -> PyResult<u64> {
        let count = self.entry_count;
        match u64::try_from(entry_id) {
            Ok(id) if id < count => Ok(id),
            _ => Err(PyErr::new::<exceptions::PyIndexError, _>(format!(
                "entry {entry_id} is out of range ({count} entries)"
            ))),
        }
    }

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
                current_record: 0,
                output_format,
                csv_header_written: false,
                skip_errors: self.skip_errors,
            },
        )
    }
}

#[pyclass(module = "mft")]
pub struct PyMftEntriesIterator {
    inner: SharedParser,
    total_number_of_records: u64,
    current_record: u64,
    output_format: Output,
    csv_header_written: bool,
    skip_errors: bool,
}

#[pymethods]
impl PyMftEntriesIterator {
    fn __iter__(slf: PyRefMut<Self>) -> PyResult<Py<PyMftEntriesIterator>> {
        Ok(slf.into())
    }

    fn __next__(&mut self, py: Python) -> PyResult<Option<Py<PyAny>>> {
        let mut parser = lock_parser(&self.inner, py);
        while self.current_record < self.total_number_of_records {
            let id = self.current_record;
            self.current_record += 1;

            let entry = match parser.get_entry(id) {
                Ok(entry) if &entry.header.signature == ZERO_HEADER => continue,
                Ok(entry) => entry,
                // The file failed to read, which skipping the entry doesn't fix.
                Err(error @ mft_rs::err::Error::IoError { .. }) => {
                    return Err(PyMftError(error).into());
                }
                Err(error) => {
                    // Unlocked while warning, as a `warnings` hook may use the parser.
                    drop(parser);
                    skip_or_raise(py, self.skip_errors, format_args!("entry {id}"), error)?;
                    parser = lock_parser(&self.inner, py);
                    continue;
                }
            };
            if let Output::Bodyfile = self.output_format {
                match Self::entry_to_bodyfile(&mut parser, &entry, py)? {
                    Some(lines) => return Ok(Some(lines)),
                    None => continue,
                }
            }
            return match self.output_format {
                Output::Python => {
                    PyMftEntry::from_mft_entry(py, entry, &mut parser, self.skip_errors)
                        .map(Py::into_any)
                }
                Output::JSON => Self::entry_to_json(entry, py),
                Output::CSV => {
                    Self::entry_to_csv(&mut self.csv_header_written, &mut parser, entry, py)
                }
                Output::Bodyfile => unreachable!("handled above"),
            }
            .map(Some);
        }
        Ok(None)
    }
}

impl PyMftEntriesIterator {
    /// The entry's bodyfile lines, or `None` if it has none (an extension record, or an entry
    /// without a `$FILE_NAME`).
    fn entry_to_bodyfile(
        parser: &mut BoxedParser,
        entry: &MftEntry,
        py: Python,
    ) -> PyResult<Option<Py<PyAny>>> {
        if !entry.header.is_base_record() {
            return Ok(None);
        }
        let extensions = parser.get_extension_entries(entry);
        // As `mft_dump -o bodyfile` does, a path that can't be resolved is left empty.
        let full_path = parser
            .get_full_path_for_entry(entry)
            .ok()
            .flatten()
            .unwrap_or_default();
        let mut lines = Vec::new();
        mft_rs::bodyfile::write_entry(&mut lines, entry, &extensions, &full_path)?;
        if lines.is_empty() {
            return Ok(None);
        }
        Ok(Some(
            PyString::new(py, &String::from_utf8_lossy(&lines))
                .into_any()
                .unbind(),
        ))
    }

    fn entry_to_json(entry: MftEntry, py: Python) -> PyResult<Py<PyAny>> {
        let json = sonic_rs::to_string(&entry).map_err(|_| {
            PyErr::new::<exceptions::PyRuntimeError, _>("JSON Serialization failed")
        })?;
        Ok(PyString::new(py, &json).into_any().unbind())
    }

    fn entry_to_csv(
        csv_header_written: &mut bool,
        parser: &mut BoxedParser,
        entry: MftEntry,
        py: Python,
    ) -> PyResult<Py<PyAny>> {
        let mut writer = WriterBuilder::new()
            .has_headers(!*csv_header_written)
            .from_writer(Vec::new());

        writer
            .serialize(FlatMftEntryWithName::from_entry(&entry, parser))
            .map_err(|_| PyErr::new::<exceptions::PyRuntimeError, _>("CSV Serialization failed"))?;
        let bytes = writer
            .into_inner()
            .map_err(|e| PyErr::new::<exceptions::PyRuntimeError, _>(e.to_string()))?;
        // Only now, so the header goes with the next record if this one failed.
        *csv_header_written = true;
        Ok(PyBytes::new(py, &bytes).into_any().unbind())
    }
}

/// A `$DATA` stream of a file, from `PyMftParser.data_streams`.
#[pyclass(frozen, module = "mft")]
pub struct PyMftDataStream {
    /// The stream's name, empty for the unnamed (main) stream.
    #[pyo3(get)]
    name: String,
    /// The stream's size in bytes.
    #[pyo3(get)]
    size: u64,
    /// The record the stream is stored in: the file's entry, or one of its extension records.
    #[pyo3(get)]
    record_entry_id: u64,
    /// The attribute's instance number in that record.
    #[pyo3(get)]
    attribute_id: u16,
    /// The content of a resident stream, `None` for a non-resident one.
    #[pyo3(get)]
    data: Option<Py<PyBytes>>,
}

#[pymethods]
impl PyMftDataStream {
    /// Whether the content is stored in the MFT record.
    #[getter]
    fn is_resident(&self) -> bool {
        self.data.is_some()
    }

    fn __repr__(&self) -> String {
        format!(
            "PyMftDataStream(name={:?}, size={}, record_entry_id={}, is_resident={})",
            self.name,
            self.size,
            self.record_entry_id,
            self.is_resident()
        )
    }
}

// Don't use double quotes ("") inside this docstring, this will crash pyo3.
/// Parses an mft file.
#[pymodule(gil_used = false)]
fn mft(m: &Bound<'_, PyModule>) -> PyResult<()> {
    init_logging(m.py()).ok();

    m.add_class::<PyMftParser>()?;
    m.add_class::<PyMftDataStream>()?;

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
