use mft_rs::attribute::x20::{AttributeListAttr, AttributeListEntry};
use pyo3::prelude::*;
use pyo3::{PyResult, Python, ToPyObject};

#[pyclass]
pub struct PyMftAttributeX20Entry {
    #[pyo3(get)]
    pub attribute_type: u32,
    #[pyo3(get)]
    pub lowest_vcn: u64,
    #[pyo3(get)]
    pub name: String,
}

impl PyMftAttributeX20Entry {
    pub fn from_x20_entry(py: Python, attr: &AttributeListEntry) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeX20Entry {
                attribute_type: attr.attribute_type,
                lowest_vcn: attr.lowest_vcn,
                name: attr.name.clone(),
            },
        )
    }
}

#[pyclass]
pub struct PyMftAttributeX20 {
    inner: AttributeListAttr,
}

#[pyclass]
pub struct PyMftX20EntriesIter {
    inner: Box<dyn Iterator<Item = PyObject> + Send>,
}

#[pymethods]
impl PyMftX20EntriesIter {
    fn next(&mut self) -> PyResult<Option<PyObject>> {
        // Extract the result out of the iterator, so iteration will return error, but can continue.
        Ok(self.inner.next())
    }

    fn __iter__(slf: PyRefMut<Self>) -> PyResult<Py<Self>> {
        Ok(slf.into())
    }

    fn __next__(mut slf: PyRefMut<Self>) -> PyResult<Option<PyObject>> {
        slf.next()
    }
}

impl PyMftAttributeX20 {
    pub fn from_x20(py: Python, attr: AttributeListAttr) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeX20 {
                inner: attr
            },
        )
    }
}

#[pymethods]
impl PyMftAttributeX20 {
    pub fn entries(&self) -> PyResult<Py<PyMftX20EntriesIter>> {
        Python::with_gil(|py| {
            let mut attributes = vec![];

            for entry in &self.inner.entries {
                match PyMftAttributeX20Entry::from_x20_entry(py, entry).map(|entry| entry.to_object(py)) {
                    Ok(obj) => { attributes.push(obj) }
                    Err(e) => { attributes.push(e.to_object(py)) }
                }
            }

            Py::new(
                py,
                PyMftX20EntriesIter {
                    inner: Box::new(attributes.into_iter())
                },
            )
        })
    }
}
