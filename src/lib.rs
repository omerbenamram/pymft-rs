#![allow(clippy::new_ret_no_self)]
#![feature(custom_attribute)]

mod attribute;
mod entry;
mod utils;

pub(crate) mod err;
pub use entry::PyMftEntry;

use mft::{MftEntry, MftParser};

use std::fs::File;
use std::io;
use std::io::{BufReader, Read, Seek, SeekFrom};

use serde_json;

use pyo3::prelude::*;

use pyo3::exceptions::{NotImplementedError, RuntimeError};
use pyo3::PyIterProtocol;

use crate::attribute::{
    PyMftAttribute, PyMftAttributeOther, PyMftAttributeX10, PyMftAttributeX20, PyMftAttributeX30,
    PyMftAttributeX40, PyMftAttributeX80, PyMftAttributeX90,
};
use crate::entry::PyMftAttributesIter;
use crate::err::PyMftError;
use crate::utils::{init_logging, FileOrFileLike};
use csv::WriterBuilder;
use mft::csv::FlatMftEntryWithName;
use pyo3::types::{PyBytes, PyString};

pub trait ReadSeek: Read + Seek {
    fn tell(&mut self) -> io::Result<u64> {
        self.seek(SeekFrom::Current(0))
    }
}

impl<T: Read + Seek> ReadSeek for T {}

pub enum Output {
    Python,
    CSV,
    JSON,
}

#[pyclass]
/// PyMftParser(self, path_or_file_like, /)
/// --
///
/// Returns an instance of the parser.
/// Works on both a path (string), or a file-like object.
pub struct PyMftParser {
    inner: Option<MftParser<Box<dyn ReadSeek>>>,
}

#[pymethods]
impl PyMftParser {
    #[new]
    fn new(obj: &PyRawObject, path_or_file_like: PyObject) -> PyResult<()> {
        let file_or_file_like = FileOrFileLike::from_pyobject(path_or_file_like)?;

        let (boxed_read_seek, size) = match file_or_file_like {
            FileOrFileLike::File(s) => {
                let file = File::open(s)?;
                let size = file.metadata()?.len();

                let reader = BufReader::with_capacity(4096, file);

                (Box::new(reader) as Box<dyn ReadSeek>, Some(size))
            }
            FileOrFileLike::FileLike(f) => (Box::new(f) as Box<dyn ReadSeek>, None),
        };

        let parser = MftParser::from_read_seek(boxed_read_seek, size).map_err(PyMftError)?;

        obj.init({
            PyMftParser {
                inner: Some(parser),
            }
        });

        Ok(())
    }

    /// number_of_entries(self, /)
    /// --
    ///
    /// Returns the total number of entries in the MFT.
    fn number_of_entries(&self) -> PyResult<u64> {
        match self.inner {
            Some(ref inner) => Ok(inner.get_entry_count()),
            None => Err(PyErr::new::<RuntimeError, _>(
                "Cannot call this method before object is initialized",
            )),
        }
    }

    /// entries(self, /)
    /// --
    ///
    /// Returns an iterator that yields the mft entries as python objects.
    fn entries(&mut self) -> PyResult<PyMftEntriesIterator> {
        self.records_iterator(Output::Python)
    }

    /// entries_json(self, /)
    /// --
    ///
    /// Returns an iterator that yields mft entries as JSON.
    fn entries_json(&mut self) -> PyResult<PyMftEntriesIterator> {
        self.records_iterator(Output::JSON)
    }

    /// entries_csv(self, /)
    /// --
    ///
    /// Returns an iterator that yields mft entries CSV lines.
    fn entries_csv(&mut self) -> PyResult<PyMftEntriesIterator> {
        self.records_iterator(Output::CSV)
    }
}

impl PyMftParser {
    fn records_iterator(&mut self, output_format: Output) -> PyResult<PyMftEntriesIterator> {
        let inner = match self.inner.take() {
            Some(inner) => inner,
            None => {
                return Err(PyErr::new::<RuntimeError, _>(
                    "PyMftParser can only be used once",
                ));
            }
        };

        let n_records = inner.get_entry_count();

        Ok(PyMftEntriesIterator {
            inner,
            total_number_of_records: n_records,
            current_record: 0,
            output_format,
            csv_header_written: false,
        })
    }
}

#[pyclass]
pub struct PyMftEntriesIterator {
    inner: MftParser<Box<dyn ReadSeek>>,
    total_number_of_records: u64,
    current_record: u64,
    output_format: Output,
    csv_header_written: bool,
}

impl PyMftEntriesIterator {
    fn entry_to_pyobject(
        &mut self,
        entry_result: Result<MftEntry, PyMftError>,
        py: Python,
    ) -> PyObject {
        match entry_result {
            Ok(entry) => {
                match PyMftEntry::from_mft_entry(py, entry, &mut self.inner)
                    .map(|entry| entry.into_object(py))
                {
                    Ok(py_mft_entry) => py_mft_entry,
                    Err(e) => e.into_object(py),
                }
            }
            Err(e) => PyErr::from(e).into_object(py),
        }
    }

    fn entry_to_json(
        &mut self,
        entry_result: Result<MftEntry, PyMftError>,
        py: Python,
    ) -> PyObject {
        match entry_result {
            Ok(entry) => match serde_json::to_string(&entry) {
                Ok(s) => PyString::new(py, &s).into_object(py),
                Err(_e) => {
                    PyErr::new::<RuntimeError, _>("JSON Serialization failed").into_object(py)
                }
            },
            Err(e) => PyErr::from(e).into_object(py),
        }
    }

    fn entry_to_csv(&mut self, entry_result: Result<MftEntry, PyMftError>, py: Python) -> PyObject {
        let mut writer = WriterBuilder::new()
            .has_headers(!self.csv_header_written)
            .from_writer(Vec::new());

        if !self.csv_header_written {
            self.csv_header_written = true
        }

        match entry_result {
            Ok(entry) => {
                match writer.serialize(FlatMftEntryWithName::from_entry(&entry, &mut self.inner)) {
                    Ok(()) => {}
                    Err(_e) => {
                        return PyErr::new::<RuntimeError, _>("CSV Serialization failed")
                            .into_object(py)
                    }
                }

                match writer.into_inner() {
                    Ok(bytes) => PyBytes::new(py, &bytes).into_object(py),
                    Err(e) => PyErr::new::<RuntimeError, _>(e.to_string()).into_object(py),
                }
            }
            Err(e) => PyErr::from(e).into_object(py),
        }
    }

    fn next(&mut self) -> PyResult<Option<PyObject>> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        if self.current_record == self.total_number_of_records {
            return Ok(None);
        }

        let entry_result = self
            .inner
            .get_entry(self.current_record)
            .map_err(PyMftError);

        self.current_record += 1;

        let entry_result = match self.output_format {
            Output::Python => self.entry_to_pyobject(entry_result, py),
            Output::JSON => self.entry_to_json(entry_result, py),
            Output::CSV => self.entry_to_csv(entry_result, py),
        };

        Ok(Some(entry_result))
    }
}

#[pyproto]
impl PyIterProtocol for PyMftParser {
    fn __iter__(mut slf: PyRefMut<Self>) -> PyResult<PyMftEntriesIterator> {
        slf.entries()
    }
    fn __next__(_slf: PyRefMut<Self>) -> PyResult<Option<PyObject>> {
        Err(PyErr::new::<NotImplementedError, _>("Using `next()` over `PyMftParser` is not supported. Try iterating over `PyMftParser(...).entries()`"))
    }
}

#[pyproto]
impl PyIterProtocol for PyMftEntriesIterator {
    fn __iter__(slf: PyRefMut<Self>) -> PyResult<Py<PyMftEntriesIterator>> {
        Ok(slf.into())
    }
    fn __next__(mut slf: PyRefMut<Self>) -> PyResult<Option<PyObject>> {
        slf.next()
    }
}

// Don't use double quotes ("") inside this docstring, this will crash pyo3.
/// Parses an mft file.
#[pymodule]
fn mft(py: Python, m: &PyModule) -> PyResult<()> {
    init_logging(py).ok();

    m.add_class::<PyMftParser>()?;

    // Entry
    m.add_class::<PyMftEntriesIterator>()?;
    m.add_class::<PyMftEntry>()?;

    // Attributes
    m.add_class::<PyMftAttribute>()?;
    m.add_class::<PyMftAttributesIter>()?;
    m.add_class::<PyMftAttributeX10>()?;
    m.add_class::<PyMftAttributeX20>()?;
    m.add_class::<PyMftAttributeX30>()?;
    m.add_class::<PyMftAttributeX40>()?;
    m.add_class::<PyMftAttributeX80>()?;
    m.add_class::<PyMftAttributeX90>()?;
    m.add_class::<PyMftAttributeOther>()?;

    Ok(())
}
