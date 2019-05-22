#![allow(clippy::new_ret_no_self)]
#![feature(custom_attribute)]

mod entry;
pub(crate) mod err;
mod utils;

pub use entry::PyMftEntry;

use mft::{MftAttribute, MftEntry, MftParser};

use pyo3::exceptions::{NotImplementedError, RuntimeError};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::types::PyString;

use pyo3::PyIterProtocol;
use pyo3_file::PyFileLikeObject;

use crate::err::PyMftError;
use crate::utils::FileOrFileLike;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::{fs, io};

pub trait ReadSeek: Read + Seek {
    fn tell(&mut self) -> io::Result<u64> {
        self.seek(SeekFrom::Current(0))
    }
}

impl<T: Read + Seek> ReadSeek for T {}

#[derive(Copy, Clone, PartialOrd, PartialEq)]
pub enum OutputFormat {
    CSV,
    XML,
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

    /// records(self, /)
    /// --
    ///
    /// Returns an iterator that yields XML records.
    fn entries(&mut self) -> PyResult<PyMftEntriesIterator> {
        self.records_iterator(OutputFormat::CSV)
    }

    /// records_csv(self, /)
    /// --
    ///
    /// Returns an iterator that yields CSV records.
    fn records_csv(&mut self) -> PyResult<PyMftEntriesIterator> {
        self.records_iterator(OutputFormat::CSV)
    }
}

impl PyMftParser {
    fn records_iterator(&mut self, output_format: OutputFormat) -> PyResult<PyMftEntriesIterator> {
        let mut inner = match self.inner.take() {
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
        })
    }
}

#[pyclass]
pub struct PyMftEntriesIterator {
    inner: MftParser<Box<dyn ReadSeek>>,
    total_number_of_records: u64,
    current_record: u64,
    output_format: OutputFormat,
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

        Ok(Some(self.entry_to_pyobject(entry_result, py)))
    }
}

#[pyproto]
impl PyIterProtocol for PyMftParser {
    fn __iter__(mut slf: PyRefMut<Self>) -> PyResult<PyMftEntriesIterator> {
        slf.entries()
    }
    fn __next__(_slf: PyRefMut<Self>) -> PyResult<Option<PyObject>> {
        Err(PyErr::new::<NotImplementedError, _>("Using `next()` over `PyMftParser` is not supported. Try iterating over `PyMftParser(...).records()`"))
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
fn mft(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyMftParser>()?;
    m.add_class::<PyMftEntriesIterator>()?;
    m.add_class::<PyMftEntry>()?;

    Ok(())
}
