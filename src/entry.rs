use crate::ReadSeek;
use pyo3::prelude::*;

use crate::attribute::PyMftAttribute;
use crate::err::PyMftError;
use mft::{MftEntry, MftParser};
use pyo3::{Py, PyIterProtocol, PyResult, Python};
use std::path::PathBuf;

#[pyclass]
pub struct PyMftEntry {
    // We need to keep inner entry to access it's attributes.
    inner: MftEntry,
    #[pyo3(get)]
    pub entry_id: u64,
    #[pyo3(get)]
    pub sequence: u16,
    #[pyo3(get)]
    pub base_entry_id: u64,
    #[pyo3(get)]
    pub base_entry_sequence: u16,
    #[pyo3(get)]
    pub hard_link_count: u16,
    #[pyo3(get)]
    pub flags: String,
    #[pyo3(get)]
    pub used_entry_size: u32,
    #[pyo3(get)]
    pub total_entry_size: u32,
    #[pyo3(get)]
    pub full_path: String,
}

#[pymethods]
impl PyMftEntry {
    pub fn attributes(&self) -> PyResult<Py<PyMftAttributesIter>> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        let mut attributes = vec![];

        for attribute_result in self.inner.iter_attributes() {
            match attribute_result {
                Ok(attribute) => match PyMftAttribute::from_mft_attribute(py, attribute)
                    .map(|entry| entry.to_object(py))
                {
                    Ok(obj) => attributes.push(obj),
                    Err(e) => attributes.push(e.to_object(py)),
                },
                Err(e) => attributes.push(PyErr::from(PyMftError(e)).to_object(py)),
            }
        }

        Py::new(
            py,
            PyMftAttributesIter {
                inner: Box::new(attributes.into_iter()),
            },
        )
    }
}

impl PyMftEntry {
    pub fn from_mft_entry(
        py: Python,
        entry: MftEntry,
        parser: &mut MftParser<impl ReadSeek>,
    ) -> PyResult<Py<PyMftEntry>> {
        let full_path = parser
            .get_full_path_for_entry(&entry)
            .expect("unreachable")
            .unwrap_or_else(|| PathBuf::from("[UNKNOWN]"))
            .to_string_lossy()
            .to_string();

        Py::new(
            py,
            PyMftEntry {
                entry_id: entry.header.record_number,
                sequence: entry.header.sequence,
                base_entry_id: entry.header.base_reference.entry,
                base_entry_sequence: 0,
                hard_link_count: 0,
                flags: format!("{:?}", entry.header.flags),
                used_entry_size: entry.header.used_entry_size,
                total_entry_size: entry.header.total_entry_size,
                inner: entry,
                full_path,
            },
        )
    }
}

#[pyclass]
pub struct PyMftAttributesIter {
    inner: Box<dyn Iterator<Item = PyObject> + Send>,
}

#[pyproto]
impl PyIterProtocol for PyMftAttributesIter {
    fn __iter__(slf: PyRefMut<Self>) -> PyResult<Py<PyMftAttributesIter>> {
        Ok(slf.into())
    }

    fn __next__(mut slf: PyRefMut<Self>) -> PyResult<Option<PyObject>> {
        slf.next()
    }
}

impl PyMftAttributesIter {
    fn next(&mut self) -> PyResult<Option<PyObject>> {
        // Extract the result out of the iterator, so iteration will return error, but can continue.
        Ok(self.inner.next())
    }
}
