use mft::attribute::header::ResidentialHeader;
use mft::attribute::{MftAttributeContent, MftAttributeType};

use mft::attribute::raw::RawAttribute;
use mft::attribute::x40::ObjectIdAttr;
use mft::attribute::x80::DataAttr;
use mft::attribute::x90::IndexRootAttr;
use mft::{FileNameAttr, MftAttribute, StandardInfoAttr};
use pyo3::ffi::Py_None;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDateTime, PyTzInfo};
use pyo3::{Py, PyResult, Python};

#[pyclass]
pub struct PyMftAttribute {
    inner: MftAttribute,
    /// Hex value of attribute type
    #[pyo3(get)]
    pub type_code: MftAttributeType,
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
    #[pyo3(get)]
    /// Will be one of
    /// - `PyMftAttributeX10`
    /// - `PyMftAttributeX30`
    /// - `PyMftAttributeX40`
    /// - `PyMftAttributeX80`
    /// - `PyMftAttributeX90`
    /// - `PyMftAttributeOther` (Currently unparsed in rust)
    pub attribute_content: PyObject,
}

impl PyMftAttribute {
    pub fn from_mft_attribute(py: Python, attr: MftAttribute) -> PyResult<Py<Self>> {
        let attr_content = match attr.data.clone() {
            MftAttributeContent::AttrX10(info) => {
                PyMftAttributeX10::from_x10(py, info)?.into_object()
            }
            MftAttributeContent::AttrX30(info) => {
                PyMftAttributeX30::from_x30(py, info)?.into_object()
            }
            MftAttributeContent::AttrX40(info) => {
                PyMftAttributeX40::from_x40(py, info)?.into_object()
            }
            MftAttributeContent::AttrX80(info) => {
                PyMftAttributeX80::from_x80(py, info)?.into_object()
            }
            MftAttributeContent::AttrX90(info) => {
                PyMftAttributeX90::from_x90(py, info)?.into_object()
            }
            MftAttributeContent::Raw(raw) => PyMftAttributeOther::from_raw(py, raw)?.into_object(),
            MftAttributeContent::None => None.into_py(py),
        };

        Py::new(
            py,
            PyMftAttribute {
                type_name: format!("{:?}", &attr.header.type_code),
                type_code: attr.header.type_code.clone(),
                name: attr.header.name.clone(),
                data_flags: format!("{:?}", &attr.header.data_flags),
                is_resident: {
                    if let Some(ResidentialHeader::Resident(_)) = attr.header.residential_header {
                        true
                    } else {
                        false
                    }
                },
                data_size: attr.header.record_length.clone(),
                attribute_content: attr_content,
                inner: attr,
            },
        )
    }
}

#[pyclass]
pub struct PyMftAttributeX10 {
    inner: StandardInfoAttr,
    #[pyo3(get)]
    pub created: PyDateTime,
    #[pyo3(get)]
    pub modified: PyDateTime,
    #[pyo3(get)]
    pub mft_modified: PyDateTime,
    #[pyo3(get)]
    pub accessed: PyDateTime,
    #[pyo3(get)]
    pub file_flags: String,
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
                created: PyDateTime::from_timestamp(py, attr.created.timestamp() as f64, None)?
                    .into(),
                modified: PyDateTime::from_timestamp(py, attr.modified.timestamp() as f64, None)?
                    .into(),
                mft_modified: PyDateTime::from_timestamp(
                    py,
                    attr.mft_modified.timestamp() as f64,
                    None,
                )?
                .into(),
                accessed: PyDateTime::from_timestamp(py, attr.accessed.timestamp() as f64, None)?
                    .into(),
                file_flags: format!("{:?}", attr.file_flags),
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

#[pyclass]
pub struct PyMftAttributeX30 {
    inner: FileNameAttr,
    #[pyo3(get)]
    pub parent_entry_id: u64,
    #[pyo3(get)]
    pub parent_entry_sequence: u16,
    #[pyo3(get)]
    pub created: PyDateTime,
    #[pyo3(get)]
    pub modified: PyDateTime,
    #[pyo3(get)]
    pub mft_modified: PyDateTime,
    #[pyo3(get)]
    pub accessed: PyDateTime,
    #[pyo3(get)]
    pub logical_size: u64,
    #[pyo3(get)]
    pub physical_size: u64,
    #[pyo3(get)]
    pub flags: String,
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
                created: PyDateTime::from_timestamp(py, attr.created.timestamp() as f64, None)?
                    .into(),
                modified: PyDateTime::from_timestamp(py, attr.modified.timestamp() as f64, None)?
                    .into(),
                mft_modified: PyDateTime::from_timestamp(
                    py,
                    attr.mft_modified.timestamp() as f64,
                    None,
                )?
                .into(),
                accessed: PyDateTime::from_timestamp(py, attr.accessed.timestamp() as f64, None)?
                    .into(),
                logical_size: attr.logical_size,
                physical_size: attr.physical_size,
                flags: format!("{:?}", &attr.flags),
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
                    .and_then(|a| a.to_string())
                    .unwrap_or_default(),
                birth_object_id: attr
                    .birth_object_id
                    .as_ref()
                    .and_then(|a| a.to_string())
                    .unwrap_or_default(),
                domain_id: attr
                    .domain_id
                    .as_ref()
                    .and_then(|a| a.to_string())
                    .unwrap_or_default(),
            },
        )
    }
}

#[pyclass]
pub struct PyMftAttributeX80 {
    #[pyo3(get)]
    data: PyBytes,
}

impl PyMftAttributeX80 {
    pub fn from_x80(py: Python, attr: DataAttr) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeX80 {
                data: PyBytes::new(py, attr.data())?,
            },
        )
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
    #[pyo3(get)]
    data: PyBytes,
}

impl PyMftAttributeOther {
    pub fn from_raw(py: Python, attr: RawAttribute) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeOther {
                data: PyBytes::new(py, &attr.data)?,
            },
        )
    }
}
