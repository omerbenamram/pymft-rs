use mft_rs::attribute::MftAttributeContent;
use mft_rs::attribute::data_run::{DataRun, RunType};
use mft_rs::attribute::header::ResidentialHeader;
use mft_rs::attribute::non_resident_attr::NonResidentAttr;
use mft_rs::attribute::raw::RawAttribute;
use mft_rs::attribute::x40::ObjectIdAttr;
use mft_rs::attribute::x80::DataAttr;
use mft_rs::attribute::x90::{IndexEntryHeader, IndexRootAttr, IndexRootFlags};
use mft_rs::{FileNameAttr, MftAttribute, StandardInfoAttr};

use num_traits::cast::ToPrimitive;

use mft_rs::attribute::x20::{AttributeListAttr, AttributeListEntry};
use pyo3::IntoPyObjectExt;
use pyo3::prelude::*;

use crate::utils::{date_to_pyobject, flags_to_string};

#[pyclass(module = "mft")]
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
                type_name: format!("{:?}", attr.header.type_code),
                type_code: attr.header.type_code.to_u32().unwrap(),
                name: attr.header.name.clone(),
                data_flags: flags_to_string(&attr.header.data_flags),
                is_resident: {
                    matches!(
                        attr.header.residential_header,
                        ResidentialHeader::Resident(_)
                    )
                },
                data_size: attr.header.record_length,
                inner: attr,
            },
        )
    }
}

#[pymethods]
impl PyMftAttribute {
    /// Will be one of
    /// - `PyMftAttributeX10`
    /// - `PyMftAttributeX20`
    /// - `PyMftAttributeX30`
    /// - `PyMftAttributeX40`
    /// - `PyMftAttributeX80`
    /// - `PyMftAttributeX90`
    /// - `PyMftAttributeNonResident` (if attribute content is non-resident)
    /// - `PyMftAttributeOther` (Currently unparsed in rust)
    /// - `None` (if the attribute has no content)
    #[getter]
    pub fn attribute_content(&self) -> PyResult<Py<PyAny>> {
        Python::attach(|py| {
            Ok(match &self.inner.data {
                MftAttributeContent::AttrX10(info) => {
                    PyMftAttributeX10::from_x10(py, info.clone())?.into_py_any(py)?
                }
                MftAttributeContent::AttrX20(info) => {
                    PyMftAttributeX20::from_x20(py, info.clone())?.into_py_any(py)?
                }
                MftAttributeContent::AttrX30(info) => {
                    PyMftAttributeX30::from_x30(py, info.clone())?.into_py_any(py)?
                }
                MftAttributeContent::AttrX40(info) => {
                    PyMftAttributeX40::from_x40(py, info.clone())?.into_py_any(py)?
                }
                MftAttributeContent::AttrX80(info) => {
                    PyMftAttributeX80::from_x80(py, info.clone())?.into_py_any(py)?
                }
                MftAttributeContent::AttrX90(info) => {
                    PyMftAttributeX90::from_x90(py, info)?.into_py_any(py)?
                }
                MftAttributeContent::Raw(raw) => {
                    PyMftAttributeOther::from_raw(py, raw.clone())?.into_py_any(py)?
                }
                // Non-resident attribute: the content lives in clusters elsewhere on the volume.
                MftAttributeContent::DataRun(info) => {
                    PyMftAttributeNonResident::from_non_resident(py, info)?.into_py_any(py)?
                }
                MftAttributeContent::None => py.None(),
            })
        })
    }
}

#[pyclass(module = "mft")]
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
    pub fn created(&self) -> PyResult<Py<PyAny>> {
        date_to_pyobject(&self.inner.created)
    }
    #[getter]
    pub fn modified(&self) -> PyResult<Py<PyAny>> {
        date_to_pyobject(&self.inner.modified)
    }
    #[getter]
    pub fn mft_modified(&self) -> PyResult<Py<PyAny>> {
        date_to_pyobject(&self.inner.mft_modified)
    }

    #[getter]
    pub fn accessed(&self) -> PyResult<Py<PyAny>> {
        date_to_pyobject(&self.inner.accessed)
    }

    #[getter]
    pub fn file_flags(&self) -> PyResult<String> {
        Ok(flags_to_string(&self.inner.file_flags))
    }
}

#[pyclass(module = "mft")]
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

#[pyclass(module = "mft")]
pub struct PyMftAttributeX20 {
    inner: AttributeListAttr,
}

#[pyclass(module = "mft")]
pub struct PyMftX20EntriesIter {
    inner: Box<dyn Iterator<Item = Py<PyAny>> + Send + Sync>,
}

#[pymethods]
impl PyMftX20EntriesIter {
    fn __iter__(slf: PyRefMut<Self>) -> PyResult<Py<Self>> {
        Ok(slf.into())
    }

    fn __next__(mut slf: PyRefMut<Self>) -> PyResult<Option<Py<PyAny>>> {
        slf.next()
    }
}

impl PyMftX20EntriesIter {
    fn next(&mut self) -> PyResult<Option<Py<PyAny>>> {
        // Extract the result out of the iterator, so iteration will return error, but can continue.
        Ok(self.inner.next())
    }
}

impl PyMftAttributeX20 {
    pub fn from_x20(py: Python, attr: AttributeListAttr) -> PyResult<Py<Self>> {
        Py::new(py, PyMftAttributeX20 { inner: attr })
    }
}

#[pymethods]
impl PyMftAttributeX20 {
    pub fn entries(&self) -> PyResult<Py<PyMftX20EntriesIter>> {
        Python::attach(|py| {
            let mut attributes = vec![];

            for entry in &self.inner.entries {
                let obj = match PyMftAttributeX20Entry::from_x20_entry(py, entry)
                    .and_then(|entry| entry.into_py_any(py))
                {
                    Ok(obj) => obj,
                    Err(e) => e.into_py_any(py).unwrap(),
                };
                attributes.push(obj);
            }

            Py::new(
                py,
                PyMftX20EntriesIter {
                    inner: Box::new(attributes.into_iter()),
                },
            )
        })
    }
}

#[pyclass(module = "mft")]
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
                namespace: format!("{:?}", attr.namespace),
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
    pub fn created(&self) -> PyResult<Py<PyAny>> {
        date_to_pyobject(&self.inner.created)
    }

    #[getter]
    pub fn modified(&self) -> PyResult<Py<PyAny>> {
        date_to_pyobject(&self.inner.modified)
    }

    #[getter]
    pub fn mft_modified(&self) -> PyResult<Py<PyAny>> {
        date_to_pyobject(&self.inner.mft_modified)
    }

    #[getter]
    pub fn accessed(&self) -> PyResult<Py<PyAny>> {
        date_to_pyobject(&self.inner.accessed)
    }

    #[getter]
    pub fn flags(&self) -> PyResult<String> {
        Ok(flags_to_string(&self.inner.flags))
    }
}

#[pyclass(module = "mft")]
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
                    .map(|a| a.to_string())
                    .unwrap_or_default(),
                birth_object_id: attr
                    .birth_object_id
                    .as_ref()
                    .map(|a| a.to_string())
                    .unwrap_or_default(),
                domain_id: attr
                    .domain_id
                    .as_ref()
                    .map(|a| a.to_string())
                    .unwrap_or_default(),
            },
        )
    }
}

#[pyclass(module = "mft")]
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
    pub fn data(&self) -> &[u8] {
        self.inner.data()
    }
}

#[pyclass(module = "mft")]
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
    #[pyo3(get)]
    pub relative_offset_to_index_node: u32,
    #[pyo3(get)]
    pub index_node_length: u32,
    #[pyo3(get)]
    pub index_node_allocation_length: u32,
    #[pyo3(get)]
    /// Whether the index continues in `$INDEX_ALLOCATION` (only the entries in the
    /// index root are available here).
    pub is_large_index: bool,
    index_entries: Vec<Py<PyMftIndexEntry>>,
}

impl PyMftAttributeX90 {
    pub fn from_x90(py: Python, attr: &IndexRootAttr) -> PyResult<Py<Self>> {
        let index_entries = attr
            .index_entries
            .index_entries
            .iter()
            .map(|entry| PyMftIndexEntry::from_index_entry(py, entry))
            .collect::<PyResult<_>>()?;

        Py::new(
            py,
            PyMftAttributeX90 {
                attribute_type: attr.attribute_type,
                collation_rule: attr.collation_rule as u32,
                index_entry_size: attr.index_entry_size,
                index_entry_number_of_cluster_blocks: attr.index_entry_number_of_cluster_blocks,
                relative_offset_to_index_node: attr.relative_offset_to_index_node,
                index_node_length: attr.index_node_length,
                index_node_allocation_length: attr.index_node_allocation_length,
                is_large_index: attr.index_root_flags.contains(IndexRootFlags::LARGE_INDEX),
                index_entries,
            },
        )
    }
}

#[pymethods]
impl PyMftAttributeX90 {
    /// The index entries stored in the index root itself.
    pub fn index_entries(&self, py: Python) -> Vec<Py<PyMftIndexEntry>> {
        self.index_entries
            .iter()
            .map(|entry| entry.clone_ref(py))
            .collect()
    }
}

/// An entry of a `$FILE_NAME` index (`$I30`), pointing at the file it names.
#[pyclass(module = "mft")]
pub struct PyMftIndexEntry {
    #[pyo3(get)]
    pub mft_reference_entry_id: u64,
    #[pyo3(get)]
    pub mft_reference_entry_sequence: u16,
    #[pyo3(get)]
    pub index_record_length: u16,
    #[pyo3(get)]
    pub attr_fname_length: u16,
    #[pyo3(get)]
    pub flags: String,
    #[pyo3(get)]
    /// The copy of the file's `$FILE_NAME` attribute kept in the index.
    pub fname_info: Py<PyMftAttributeX30>,
}

impl PyMftIndexEntry {
    pub fn from_index_entry(py: Python, entry: &IndexEntryHeader) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftIndexEntry {
                mft_reference_entry_id: entry.mft_reference.entry,
                mft_reference_entry_sequence: entry.mft_reference.sequence,
                index_record_length: entry.index_record_length,
                attr_fname_length: entry.attr_fname_length,
                flags: flags_to_string(&entry.flags),
                fname_info: PyMftAttributeX30::from_x30(py, entry.fname_info.clone())?,
            },
        )
    }
}

/// The content of a non-resident attribute: the data runs that locate it on the volume.
#[pyclass(module = "mft")]
pub struct PyMftAttributeNonResident {
    inner: NonResidentAttr,
}

impl PyMftAttributeNonResident {
    pub fn from_non_resident(py: Python, attr: &NonResidentAttr) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeNonResident {
                inner: attr.clone(),
            },
        )
    }
}

#[pymethods]
impl PyMftAttributeNonResident {
    pub fn data_runs(&self, py: Python) -> PyResult<Vec<Py<PyMftDataRun>>> {
        self.inner
            .data_runs
            .iter()
            .map(|run| PyMftDataRun::from_data_run(py, run))
            .collect()
    }
}

/// A contiguous run of clusters. `lcn_offset` is the absolute cluster number where the run
/// starts, and is 0 for sparse runs, which have no clusters on disk.
#[pyclass(module = "mft")]
pub struct PyMftDataRun {
    #[pyo3(get)]
    pub lcn_offset: u64,
    #[pyo3(get)]
    pub lcn_length: u64,
    #[pyo3(get)]
    pub is_sparse: bool,
}

impl PyMftDataRun {
    pub fn from_data_run(py: Python, run: &DataRun) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftDataRun {
                lcn_offset: run.lcn_offset,
                lcn_length: run.lcn_length,
                is_sparse: run.run_type == RunType::Sparse,
            },
        )
    }
}

#[pyclass(module = "mft")]
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
    pub fn data(&self) -> &[u8] {
        &self.inner.data
    }
}
