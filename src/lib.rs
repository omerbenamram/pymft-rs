#![allow(clippy::new_ret_no_self)]
mod entry;

pub use entry::PyMftEntry;

use mft::{MftAttribute, MftEntry, MftParser};

use pyo3::exceptions::{NotImplementedError, RuntimeError};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::types::PyString;

use pyo3::PyIterProtocol;
use pyo3_file::PyFileLikeObject;

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::{fs, io};

pub trait ReadSeek: Read + Seek {
    fn tell(&mut self) -> io::Result<u64> {
        self.seek(SeekFrom::Current(0))
    }
}

impl<T: Read + Seek> ReadSeek for T {}

struct PyMftError(mft::err::Error);

impl From<PyMftError> for PyErr {
    fn from(err: PyMftError) -> Self {
        match err.0 {
            mft::err::Error::IoError {
                source,
                backtrace: _,
            } => source.into(),
            _ => PyErr::new::<RuntimeError, _>(format!("{}", err.0)),
        }
    }
}

#[derive(Copy, Clone, PartialOrd, PartialEq)]
pub enum OutputFormat {
    CSV,
    XML,
}

#[derive(Debug)]
enum FileOrFileLike {
    File(String),
    FileLike(PyFileLikeObject),
}

impl FileOrFileLike {
    pub fn from_pyobject(path_or_file_like: PyObject) -> PyResult<FileOrFileLike> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        // is a path
        if let Ok(string_ref) = path_or_file_like.cast_as::<PyString>(py) {
            return Ok(FileOrFileLike::File(
                string_ref.to_string_lossy().to_string(),
            ));
        }

        // We only need read + seek
        match PyFileLikeObject::with_requirements(path_or_file_like, true, false, true) {
            Ok(f) => Ok(FileOrFileLike::FileLike(f)),
            Err(e) => Err(e),
        }
    }
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
                let size = fs::metadata(file)?.len();

                let reader = BufReader::with_capacity(4096, file);

                (Box::new(reader) as Box<dyn ReadSeek>, Some(size))
            }
            FileOrFileLike::FileLike(f) => (Box::new(f) as Box<dyn ReadSeek>, None),
        };

        let parser = MftParser::from_read_seek(boxed_read_seek, size).map_err(PyEvtxError)?;

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

        Ok(PyMftEntriesIterator {
            inner: inner.iter_entries().into_iter(),
            records: None,
            output_format,
        })
    }
}

fn record_to_pydict(record: SerializedEvtxRecord, py: Python) -> PyResult<&PyDict> {
    let pyrecord = PyDict::new(py);

    pyrecord.set_item("event_record_id", record.event_record_id)?;
    pyrecord.set_item("timestamp", format!("{}", record.timestamp))?;
    pyrecord.set_item("data", record.data)?;
    Ok(pyrecord)
}

fn record_to_pyobject(
    r: Result<SerializedEvtxRecord, evtx::err::Error>,
    py: Python,
) -> PyResult<PyObject> {
    match r {
        Ok(r) => match record_to_pydict(r, py) {
            Ok(dict) => Ok(dict.to_object(py)),
            Err(e) => Ok(e.to_object(py)),
        },
        Err(e) => Err(PyEvtxError(e).into()),
    }
}



#[pyclass]
pub struct PyMftEntriesIterator {
    inner: IntoIterChunks<Box<dyn ReadSeek>>,
    records: Option<Vec<Result<MftEntry, mft::err::Error>>>,
    output_format: OutputFormat,
}

impl PyMftEntriesIterator {
    fn next(&mut self) -> PyResult<Option<PyObject>> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        loop {
            if let Some(record) = self.records.as_mut().and_then(Vec::pop) {
                return record_to_pyobject(record, py).map(Some);
            }

            let chunk = self.inner.next();

            match chunk {
                None => return Ok(None),
                Some(chunk_result) => match chunk_result {
                    Err(e) => {
                        return Err(PyEvtxError(e).into());
                    }
                    Ok(mut chunk) => {
                        let parsed_chunk = chunk.parse(&self.settings);

                        match parsed_chunk {
                            Err(e) => {
                                return Err(PyEvtxError(e).into());
                            }
                            Ok(mut chunk) => {
                                self.records = match self.output_format {
                                    OutputFormat::XML => Some(
                                        chunk
                                            .iter_serialized_records::<XmlOutput<Vec<u8>>>()
                                            .collect(),
                                    ),
                                    OutputFormat::JSON => Some(
                                        chunk
                                            .iter_serialized_records::<JsonOutput<Vec<u8>>>()
                                            .collect(),
                                    ),
                                };
                            }
                        }
                    }
                },
            }
        }
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
