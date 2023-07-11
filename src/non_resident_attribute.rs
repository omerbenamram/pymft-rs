use serde::Serialize;

use mft_rs::attribute::non_resident_attr::NonResidentAttr;
use mft_rs::attribute::data_run::{DataRun, RunType};
use pyo3::prelude::*;
use pyo3::{Py, PyResult, Python, ToPyObject};
use pyo3::types::PyString;
use pyo3::exceptions::PyRuntimeError;

#[pyclass]
#[derive(Serialize, Clone, Debug)]
pub struct PyMftAttributeNonResidentDataRun {
    #[pyo3(get)]
    pub lcn_offset: u64,
    #[pyo3(get)]
    pub lcn_length: u64,
    #[pyo3(get)]
    pub is_sparse: bool,
}

impl PyMftAttributeNonResidentDataRun {
    pub fn from_non_resident_data_run(py: Python, data_run: &DataRun) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeNonResidentDataRun {
                lcn_offset: data_run.lcn_offset,
                lcn_length: data_run.lcn_length,
                is_sparse: data_run.run_type == RunType::Sparse
            },
        )
    }
}

#[pyclass]
pub struct PyMftAttributeNonResident {
    inner: NonResidentAttr,
}

#[pyclass]
pub struct PyMftAttributeNonResidentDataRunsIter {
    inner: Box<dyn Iterator<Item = PyObject> + Send>
}

#[pymethods]
impl PyMftAttributeNonResidentDataRunsIter {
    fn next(&mut self) -> PyResult<Option<PyObject>> {
        Ok(self.inner.next())
    }

    fn __iter__(slf: PyRefMut<Self>) -> PyResult<Py<Self>> {
        Ok(slf.into())
    }

    fn __next__(mut slf: PyRefMut<Self>) -> PyResult<Option<PyObject>> {
        slf.next()
    }
}

impl PyMftAttributeNonResident {
    pub fn from_data_run(py: Python, attr: NonResidentAttr) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeNonResident {
                inner: attr
            },
        )
    }
}

#[pymethods]
impl PyMftAttributeNonResident {
    pub fn data_runs(&self) -> PyResult<Py<PyMftAttributeNonResidentDataRunsIter>> {
        Python::with_gil(|py| {

            let mut data_runs = vec![];

            for data_run in &self.inner.data_runs {
                match PyMftAttributeNonResidentDataRun::from_non_resident_data_run(py, data_run).map(|data_run| data_run.to_object(py)) {
                    Ok(obj) => { data_runs.push(obj) }
                    Err(e) => { data_runs.push(e.to_object(py)) }
                }
            }

            Py::new(
                py,
                PyMftAttributeNonResidentDataRunsIter {
                    inner: Box::new(data_runs.into_iter())
                },
            )
        })
    }

    pub fn data_runs_json(&mut self) -> PyObject {
        Python::with_gil(|py| {
            let mut data_runs = vec![];

            for data_run in &self.inner.data_runs {
                data_runs.push(data_run);
            }

            let ret = match serde_json::to_string(&data_runs) {
                Ok(s) => PyString::new(py, &s).to_object(py),
                Err(_e) => PyErr::new::<PyRuntimeError, _>("JSON Serialization failed1").to_object(py),
            };

            return ret;
        })
    }
}

#[test]
fn test_data_runs_json() {
    let mut py_mft_attribute_non_resident = PyMftAttributeNonResident {
        inner: NonResidentAttr{
            data_runs: vec![
                DataRun {lcn_length: 0x30, lcn_offset: 0x20, run_type: RunType::Standard},
                DataRun {lcn_length: 0x60, lcn_offset: 0, run_type: RunType::Sparse},
                DataRun {lcn_length: 0x10, lcn_offset: 0x30, run_type: RunType::Standard},
            ]
        }
    };
    Python::with_gil(|py| {
        let json: std::result::Result<String, pyo3::PyErr> = py_mft_attribute_non_resident.data_runs_json().extract(py);
        assert_eq!(json.unwrap(), "[{\"lcn_offset\":32,\"lcn_length\":48,\"run_type\":\"Standard\"},{\"lcn_offset\":0,\"lcn_length\":96,\"run_type\":\"Sparse\"},{\"lcn_offset\":48,\"lcn_length\":16,\"run_type\":\"Standard\"}]")
    });
}
