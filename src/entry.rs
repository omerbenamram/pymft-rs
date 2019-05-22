use crate::{PyMftEntriesIterator, ReadSeek};
use pyo3::prelude::*;

use crate::attribute::PyMftAttribute;
use crate::err::PyMftError;
use mft::{MftAttribute, MftEntry, MftParser};
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

        Py::new(
            py,
            PyMftAttributesIter {
                inner: self.inner.iter_attributes(),
            },
        )
    }
}

#[pyproto]
impl PyIterProtocol for PyMftAttributesIter {
    fn __iter__(mut slf: PyRefMut<Self>) -> PyResult<PyMftEntriesIterator> {
        Ok(slf.into())
    }
    fn __next__(slf: PyRefMut<Self>) -> PyResult<Option<PyObject>> {
        slf.next()
    }
}

#[pyclass]
struct PyMftAttributesIter {
    inner: Box<Iterator<Item = Result<MftAttribute, PyMftError>>>,
}

impl PyMftAttributesIter {
    fn attribute_to_pyobject(
        &mut self,
        attribute_result: Result<MftAttribute, PyMftError>,
        py: Python,
    ) -> PyObject {
        match attribute_result {
            Ok(attribute) => {
                match PyMftAttribute::from_mft_attribute(py, attribute)
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

        Ok(self
            .inner
            .next()
            .map(|attribute| self.attribute_to_pyobject(attribute, py)))
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
                entry_id: entry.header.record_number.clone(),
                sequence: entry.header.sequence.clone(),
                base_entry_id: entry.header.base_reference.entry.clone(),
                base_entry_sequence: 0,
                hard_link_count: 0,
                flags: format!("{:?}", entry.header.flags),
                used_entry_size: entry.header.used_entry_size.clone(),
                total_entry_size: entry.header.total_entry_size.clone(),
                inner: entry,
                full_path,
            },
        )
    }
}