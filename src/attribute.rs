use mft::attribute::header::ResidentialHeader;
use mft::attribute::{MftAttributeContent};

use mft::attribute::raw::RawAttribute;
use mft::attribute::x40::ObjectIdAttr;
use mft::attribute::x80::DataAttr;
use mft::attribute::x90::IndexRootAttr;
use mft::{FileNameAttr, MftAttribute, StandardInfoAttr};

use num_traits::cast::ToPrimitive;

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDateTime};
use pyo3::{ffi, Py, PyResult, Python};

#[pyclass]
pub struct PyMftAttribute {
    inner: MftAttribute,
    /// Hex value of attribute type
    #[pyo3(get)]
    pub type_code: u32,
    /// String value of attribute type
    #[pyo3(get)]
    pub type_name: String,
    /// Attribute name (can be empty)
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub data_flags: String,
    #[pyo3(get)]
    pub is_resident: bool,
    #[pyo3(get)]
    pub data_size: u32,
}

impl PyMftAttribute {
    pub fn from_mft_attribute(py: Python, attr: MftAttribute) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttribute {
                type_name: format!("{:?}", &attr.header.type_code),
                type_code: attr.header.type_code.to_u32().unwrap(),
                name: attr.header.name.clone(),
                data_flags: format!("{:?}", &attr.header.data_flags),
                is_resident: {
                    if let ResidentialHeader::Resident(_) = attr.header.residential_header {
                        true
                    } else {
                        false
                    }
                },
                data_size: attr.header.record_length.clone(),
                inner: attr,
            },
        )
    }
}

#[pymethods]
impl PyMftAttribute {
    /// Will be one of
    /// - `PyMftAttributeX10`
    /// - `PyMftAttributeX30`
    /// - `PyMftAttributeX40`
    /// - `PyMftAttributeX80`
    /// - `PyMftAttributeX90`
    /// - `PyMftAttributeOther` (Currently unparsed in rust)
    /// - `None` (if attribute content is non-resident)
    pub fn attribute_content(&self) -> PyResult<PyObject> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        Ok(match &self.inner.data {
            MftAttributeContent::AttrX10(info) => {
                PyMftAttributeX10::from_x10(py, info.clone())?.into_object(py)
            }
            MftAttributeContent::AttrX30(info) => {
                PyMftAttributeX30::from_x30(py, info.clone())?.into_object(py)
            }
            MftAttributeContent::AttrX40(info) => {
                PyMftAttributeX40::from_x40(py, info.clone())?.into_object(py)
            }
            MftAttributeContent::AttrX80(info) => {
                PyMftAttributeX80::from_x80(py, info.clone())?.into_object(py)
            }
            MftAttributeContent::AttrX90(info) => {
                PyMftAttributeX90::from_x90(py, info.clone())?.into_object(py)
            }
            MftAttributeContent::Raw(raw) => {
                PyMftAttributeOther::from_raw(py, raw.clone())?.into_object(py)
            }
            MftAttributeContent::None => unsafe { PyObject::from_borrowed_ptr(py, ffi::Py_None()) },
        })
    }
}

#[pyclass]
pub struct PyMftAttributeX10 {
    inner: StandardInfoAttr,
    #[pyo3(get)]
    pub max_version: u32,
    #[pyo3(get)]
    pub version: u32,
    #[pyo3(get)]
    pub class_id: u32,
    #[pyo3(get)]
    pub owner_id: u32,
    #[pyo3(get)]
    pub security_id: u32,
    #[pyo3(get)]
    pub quota: u64,
    #[pyo3(get)]
    pub usn: u64,
}

impl PyMftAttributeX10 {
    pub fn from_x10(py: Python, attr: StandardInfoAttr) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeX10 {
                max_version: attr.max_version,
                version: attr.version,
                class_id: attr.class_id,
                owner_id: attr.owner_id,
                security_id: attr.security_id,
                quota: attr.quota,
                usn: attr.usn,
                inner: attr,
            },
        )
    }
}

#[pymethods]
impl PyMftAttributeX10 {
    #[getter]
    pub fn created(&self) -> PyResult<Py<PyDateTime>> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        PyDateTime::from_timestamp(py, self.inner.created.timestamp() as f64, None)
    }
    #[getter]
    pub fn modified(&self) -> PyResult<Py<PyDateTime>> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        PyDateTime::from_timestamp(py, self.inner.modified.timestamp() as f64, None)
    }
    #[getter]
    pub fn mft_modified(&self) -> PyResult<Py<PyDateTime>> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        PyDateTime::from_timestamp(py, self.inner.mft_modified.timestamp() as f64, None)
    }

    #[getter]
    pub fn accessed(&self) -> PyResult<Py<PyDateTime>> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        PyDateTime::from_timestamp(py, self.inner.accessed.timestamp() as f64, None)
    }

    #[getter]
    pub fn file_flags(&self) -> PyResult<String> {
        Ok(format!("{:?}", self.inner.file_flags))
    }
}

#[pyclass]
pub struct PyMftAttributeX30 {
    inner: FileNameAttr,
    #[pyo3(get)]
    pub parent_entry_id: u64,
    #[pyo3(get)]
    pub parent_entry_sequence: u16,
    #[pyo3(get)]
    pub logical_size: u64,
    #[pyo3(get)]
    pub physical_size: u64,
    #[pyo3(get)]
    pub reparse_value: u32,
    #[pyo3(get)]
    pub namespace: String,
    #[pyo3(get)]
    pub name: String,
}

impl PyMftAttributeX30 {
    pub fn from_x30(py: Python, attr: FileNameAttr) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeX30 {
                logical_size: attr.logical_size,
                physical_size: attr.physical_size,
                reparse_value: attr.reparse_value,
                namespace: format!("{:?}", &attr.namespace),
                parent_entry_id: attr.parent.entry,
                parent_entry_sequence: attr.parent.sequence,
                name: attr.name.clone(),
                inner: attr,
            },
        )
    }
}

#[pymethods]
impl PyMftAttributeX30 {
    #[getter]
    pub fn created(&self) -> PyResult<Py<PyDateTime>> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        PyDateTime::from_timestamp(py, self.inner.created.timestamp() as f64, None)
    }
    #[getter]
    pub fn modified(&self) -> PyResult<Py<PyDateTime>> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        PyDateTime::from_timestamp(py, self.inner.modified.timestamp() as f64, None)
    }
    #[getter]
    pub fn mft_modified(&self) -> PyResult<Py<PyDateTime>> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        PyDateTime::from_timestamp(py, self.inner.mft_modified.timestamp() as f64, None)
    }

    #[getter]
    pub fn accessed(&self) -> PyResult<Py<PyDateTime>> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        PyDateTime::from_timestamp(py, self.inner.accessed.timestamp() as f64, None)
    }

    #[getter]
    pub fn flags(&self) -> PyResult<String> {
        Ok(format!("{:?}", self.inner.flags))
    }
}

#[pyclass]
pub struct PyMftAttributeX40 {
    #[pyo3(get)]
    /// Unique Id assigned to file
    pub object_id: String,
    #[pyo3(get)]
    /// Volume where file was created
    pub birth_volume_id: String,
    #[pyo3(get)]
    /// Original Object Id of file
    pub birth_object_id: String,
    #[pyo3(get)]
    /// Domain in which object was created
    pub domain_id: String,
}

impl PyMftAttributeX40 {
    pub fn from_x40(py: Python, attr: ObjectIdAttr) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeX40 {
                object_id: attr.object_id.to_string(),
                birth_volume_id: attr
                    .birth_volume_id
                    .as_ref()
                    .and_then(|a| Some(a.to_string()))
                    .unwrap_or_default(),
                birth_object_id: attr
                    .birth_object_id
                    .as_ref()
                    .and_then(|a| Some(a.to_string()))
                    .unwrap_or_default(),
                domain_id: attr
                    .domain_id
                    .as_ref()
                    .and_then(|a| Some(a.to_string()))
                    .unwrap_or_default(),
            },
        )
    }
}

#[pyclass]
pub struct PyMftAttributeX80 {
    inner: DataAttr,
}

impl PyMftAttributeX80 {
    pub fn from_x80(py: Python, attr: DataAttr) -> PyResult<Py<Self>> {
        Py::new(py, PyMftAttributeX80 { inner: attr })
    }
}

#[pymethods]
impl PyMftAttributeX80 {
    #[getter]
    pub fn data(&self) -> PyResult<Py<PyBytes>> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        Ok(PyBytes::new(py, &self.inner.data()))
    }
}

#[pyclass]
pub struct PyMftAttributeX90 {
    #[pyo3(get)]
    /// Unique Id assigned to file
    pub attribute_type: u32,
    #[pyo3(get)]
    /// Collation rule used to sort the index entries.
    /// If type is $FILENAME, this must be COLLATION_FILENAME
    pub collation_rule: u32,
    #[pyo3(get)]
    /// The index entry size
    pub index_entry_size: u32,
    #[pyo3(get)]
    /// The index entry number of cluster blocks
    pub index_entry_number_of_cluster_blocks: u32,
}

impl PyMftAttributeX90 {
    pub fn from_x90(py: Python, attr: IndexRootAttr) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeX90 {
                attribute_type: attr.attribute_type,
                collation_rule: attr.collation_rule,
                index_entry_size: attr.index_entry_size,
                index_entry_number_of_cluster_blocks: attr.index_entry_number_of_cluster_blocks,
            },
        )
    }
}

#[pyclass]
pub struct PyMftAttributeOther {
    inner: RawAttribute,
}

impl PyMftAttributeOther {
    pub fn from_raw(py: Python, attr: RawAttribute) -> PyResult<Py<Self>> {
        Py::new(py, PyMftAttributeOther { inner: attr })
    }
}

#[pymethods]
impl PyMftAttributeOther {
    #[getter]
    pub fn data(&self) -> PyResult<Py<PyBytes>> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        Ok(PyBytes::new(py, &self.inner.data))
    }
}
