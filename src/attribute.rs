use std::sync::Arc;

use jiff::Timestamp;
use mft_rs::attribute::MftAttributeContent;
use mft_rs::attribute::data_run::{DataRun, RunType};
use mft_rs::attribute::header::NonResidentHeader;
use mft_rs::attribute::header::{MftAttributeHeader, ResidentialHeader};
use mft_rs::attribute::non_resident_attr::NonResidentAttr;
use mft_rs::attribute::x40::ObjectIdAttr;
use mft_rs::attribute::x50::{Ace, Acl, SecurityDescriptorAttr};
use mft_rs::attribute::x60::VolumeNameAttr;
use mft_rs::attribute::x70::VolumeInformationAttr;
use mft_rs::attribute::x90::{
    IndexEntryHeader, IndexRootAttr, IndexRootFlags, ViewIndexEntry, ViewIndexValue,
};
use mft_rs::attribute::xc0::{ReparseData, ReparsePointAttr};
use mft_rs::attribute::xd0::EaInformationAttr;
use mft_rs::attribute::xe0::{EaAttr, EaEntry, EaValue};
use mft_rs::{FileNameAttr, MftEntry, StandardInfoAttr};

use num_traits::cast::ToPrimitive;

use mft_rs::attribute::FileAttributeFlags;
use mft_rs::attribute::x20::{AttributeListAttr, AttributeListEntry};
use pyo3::IntoPyObjectExt;
use pyo3::prelude::*;

use crate::err::PyMftError;
use crate::utils::date_to_pyobject;

#[pyclass(module = "mft")]
pub struct PyMftAttribute {
    /// Attributes borrow from their entry's record, so keep the entry and where the attribute's
    /// record is in it, and re-parse the content when it is asked for.
    entry: Arc<MftEntry>,
    start_offset: usize,
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
    /// `data_flags` as an integer.
    #[pyo3(get)]
    pub data_flags_value: u16,
    #[pyo3(get)]
    pub is_resident: bool,
    #[pyo3(get)]
    pub data_size: u32,
}

impl PyMftAttribute {
    pub fn from_header(
        py: Python,
        entry: &Arc<MftEntry>,
        header: &MftAttributeHeader,
    ) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttribute {
                type_name: format!("{:?}", header.type_code),
                type_code: header.type_code.to_u32().unwrap(),
                name: header.name.to_utf8_string(),
                data_flags: header.data_flags.to_string(),
                data_flags_value: header.data_flags.bits(),
                is_resident: matches!(header.residential_header, ResidentialHeader::Resident(_)),
                data_size: header.record_length,
                entry: Arc::clone(entry),
                start_offset: header.start_offset as usize,
            },
        )
    }
}

impl PyMftAttribute {
    /// The attribute's record and its header, parsed again from the entry.
    fn header(&self) -> PyResult<(&[u8], MftAttributeHeader<'_>)> {
        // `PyMftEntry::attributes` already parsed this record's header, so it is in bounds.
        let record = &self.entry.data[self.start_offset..][..self.data_size as usize];
        let header = MftAttributeHeader::from_slice(record, self.start_offset as u64)
            .map_err(PyMftError)?
            .expect("an $END marker is not an attribute");
        Ok((record, header))
    }
}

#[pymethods]
impl PyMftAttribute {
    /// The value of a resident attribute as stored in the record, or `None` for a non-resident
    /// one. Parsed classes don't keep every byte (reserved fields, slack after names), so this
    /// is how to get them.
    #[getter]
    pub fn raw_value(&self) -> PyResult<Option<Vec<u8>>> {
        let (record, header) = self.header()?;
        Ok(match &header.residential_header {
            ResidentialHeader::Resident(resident) => {
                Some(resident.value(record).map_err(PyMftError)?.to_vec())
            }
            ResidentialHeader::NonResident(_) => None,
        })
    }

    /// Will be one of
    /// - `PyMftAttributeX10`
    /// - `PyMftAttributeX20`
    /// - `PyMftAttributeX30`
    /// - `PyMftAttributeX40`
    /// - `PyMftAttributeX50`
    /// - `PyMftAttributeX60`
    /// - `PyMftAttributeX70`
    /// - `PyMftAttributeX80`
    /// - `PyMftAttributeX90`
    /// - `PyMftAttributeXC0`
    /// - `PyMftAttributeXD0`
    /// - `PyMftAttributeXE0`
    /// - `PyMftAttributeNonResident` (if attribute content is non-resident)
    /// - `PyMftAttributeOther` (Currently unparsed in rust)
    #[getter]
    pub fn attribute_content(&self) -> PyResult<Py<PyAny>> {
        let (record, header) = self.header()?;
        let content = MftAttributeContent::from_record(record, &header).map_err(PyMftError)?;

        Python::attach(|py| {
            Ok(match &content {
                MftAttributeContent::AttrX10(info) => {
                    PyMftAttributeX10::from_x10(py, info.clone())?.into_py_any(py)?
                }
                MftAttributeContent::AttrX20(info) => {
                    PyMftAttributeX20::from_x20(py, info)?.into_py_any(py)?
                }
                MftAttributeContent::AttrX30(info) => {
                    PyMftAttributeX30::from_x30(py, info)?.into_py_any(py)?
                }
                MftAttributeContent::AttrX40(info) => {
                    PyMftAttributeX40::from_x40(py, info.clone())?.into_py_any(py)?
                }
                MftAttributeContent::AttrX50(info) => {
                    PyMftAttributeX50::from_x50(py, info)?.into_py_any(py)?
                }
                MftAttributeContent::AttrX80(info) => {
                    PyMftAttributeX80::from_data(py, info.data())?.into_py_any(py)?
                }
                MftAttributeContent::AttrX90(info) => {
                    PyMftAttributeX90::from_x90(py, info)?.into_py_any(py)?
                }
                MftAttributeContent::Raw(raw) => {
                    PyMftAttributeOther::from_data(py, raw.data)?.into_py_any(py)?
                }
                MftAttributeContent::AttrX60(info) => {
                    PyMftAttributeX60::from_x60(py, info)?.into_py_any(py)?
                }
                MftAttributeContent::AttrX70(info) => {
                    PyMftAttributeX70::from_x70(py, info)?.into_py_any(py)?
                }
                MftAttributeContent::AttrXC0(info) => {
                    PyMftAttributeXC0::from_xc0(py, info)?.into_py_any(py)?
                }
                MftAttributeContent::AttrXD0(info) => {
                    PyMftAttributeXD0::from_xd0(py, info)?.into_py_any(py)?
                }
                MftAttributeContent::AttrXE0(info) => {
                    PyMftAttributeXE0::from_xe0(py, info)?.into_py_any(py)?
                }
                // Non-resident attribute: the content lives in clusters elsewhere on the volume.
                MftAttributeContent::DataRun(info) => {
                    let ResidentialHeader::NonResident(non_resident) = &header.residential_header
                    else {
                        unreachable!("only non-resident attributes have data runs");
                    };
                    PyMftAttributeNonResident::from_non_resident(py, non_resident, info)?
                        .into_py_any(py)?
                }
            })
        })
    }
}

/// $STANDARD_INFORMATION: the file's timestamps and flags and, since NTFS 3.0, its quota owner and
/// charge, security id and update sequence number. The older layout lacks those four, which read
/// as 0. Its `raw_value` is 48 bytes long, and the newer layout's at least 72.
#[pyclass(module = "mft")]
pub struct PyMftAttributeX10 {
    inner: StandardInfoAttr,
    #[pyo3(get)]
    pub max_version: u32,
    #[pyo3(get)]
    pub version: u32,
    #[pyo3(get)]
    pub class_id: u32,
    /// The owner's key in `$Quota`, or 0 if the file has none.
    #[pyo3(get)]
    pub owner_id: u32,
    /// The key of the file's security descriptor in `$Secure`, or 0 if the descriptor is in the
    /// file's own `$SECURITY_DESCRIPTOR`.
    #[pyo3(get)]
    pub security_id: u32,
    /// The bytes charged to the owner's quota, or 0 if none are.
    #[pyo3(get)]
    pub quota: u64,
    /// The update sequence number of the last change journal (`$UsnJrnl`) record written for the
    /// file, or 0 if none was.
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

    /// The file's `FILE_ATTRIBUTE_*` flags.
    #[getter]
    pub fn file_flags(&self) -> PyResult<String> {
        Ok(self.inner.file_flags.to_string())
    }

    /// `file_flags` as an integer.
    #[getter]
    pub fn file_flags_value(&self) -> u32 {
        self.inner.file_flags.bits()
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
    /// The entry that holds the attribute: this entry, or one of its extension records.
    #[pyo3(get)]
    pub segment_entry_id: u64,
    #[pyo3(get)]
    pub segment_entry_sequence: u16,
}

impl PyMftAttributeX20Entry {
    pub fn from_x20_entry(py: Python, attr: &AttributeListEntry) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeX20Entry {
                attribute_type: attr.attribute_type,
                lowest_vcn: attr.lowest_vcn,
                name: attr.name.to_utf8_string(),
                segment_entry_id: attr.segment_reference.entry,
                segment_entry_sequence: attr.segment_reference.sequence,
            },
        )
    }
}

#[pyclass(module = "mft")]
pub struct PyMftAttributeX20 {
    entries: Vec<Py<PyAny>>,
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
    pub fn from_x20(py: Python, attr: &AttributeListAttr) -> PyResult<Py<Self>> {
        let entries = attr
            .entries
            .iter()
            .map(|entry| PyMftAttributeX20Entry::from_x20_entry(py, entry)?.into_py_any(py))
            .collect::<PyResult<_>>()?;
        Py::new(py, PyMftAttributeX20 { entries })
    }
}

#[pymethods]
impl PyMftAttributeX20 {
    pub fn entries(&self) -> PyResult<Py<PyMftX20EntriesIter>> {
        Python::attach(|py| {
            let attributes: Vec<Py<PyAny>> = self
                .entries
                .iter()
                .map(|entry| entry.clone_ref(py))
                .collect();

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
    created: Timestamp,
    modified: Timestamp,
    mft_modified: Timestamp,
    accessed: Timestamp,
    file_flags: FileAttributeFlags,
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
    pub fn from_x30(py: Python, attr: &FileNameAttr) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeX30 {
                created: attr.created,
                modified: attr.modified,
                mft_modified: attr.mft_modified,
                accessed: attr.accessed,
                file_flags: attr.flags.clone(),
                logical_size: attr.logical_size,
                physical_size: attr.physical_size,
                reparse_value: attr.reparse_value,
                namespace: format!("{:?}", attr.namespace),
                parent_entry_id: attr.parent.entry,
                parent_entry_sequence: attr.parent.sequence,
                name: attr.name.to_utf8_string(),
            },
        )
    }
}

#[pymethods]
impl PyMftAttributeX30 {
    #[getter]
    pub fn created(&self) -> PyResult<Py<PyAny>> {
        date_to_pyobject(&self.created)
    }

    #[getter]
    pub fn modified(&self) -> PyResult<Py<PyAny>> {
        date_to_pyobject(&self.modified)
    }

    #[getter]
    pub fn mft_modified(&self) -> PyResult<Py<PyAny>> {
        date_to_pyobject(&self.mft_modified)
    }

    #[getter]
    pub fn accessed(&self) -> PyResult<Py<PyAny>> {
        date_to_pyobject(&self.accessed)
    }

    #[getter]
    pub fn flags(&self) -> PyResult<String> {
        Ok(self.file_flags.to_string())
    }

    /// `flags` as an integer.
    #[getter]
    pub fn flags_value(&self) -> u32 {
        self.file_flags.bits()
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

/// $SECURITY_DESCRIPTOR: a resident security descriptor. Since NTFS 3.0 most files only have a
/// security id in `$STANDARD_INFORMATION`, and the descriptor is in `$Secure`.
#[pyclass(module = "mft")]
pub struct PyMftAttributeX50 {
    #[pyo3(get)]
    pub revision: u8,
    #[pyo3(get)]
    pub control: String,
    /// `control` as an integer.
    #[pyo3(get)]
    pub control_value: u16,
    /// The owner, as a SID string (`S-1-5-18`), or `None` when the descriptor has none.
    #[pyo3(get)]
    pub owner: Option<String>,
    #[pyo3(get)]
    pub group: Option<String>,
    /// Only set when `control` has `SE_SACL_PRESENT`.
    #[pyo3(get)]
    pub sacl: Option<Py<PyMftAcl>>,
    /// Only set when `control` has `SE_DACL_PRESENT`. A present flag without a DACL is a NULL
    /// DACL, which grants everyone full access.
    #[pyo3(get)]
    pub dacl: Option<Py<PyMftAcl>>,
}

impl PyMftAttributeX50 {
    pub fn from_x50(py: Python, attr: &SecurityDescriptorAttr) -> PyResult<Py<Self>> {
        let acl = |acl: &Option<Acl>| {
            acl.as_ref()
                .map(|acl| PyMftAcl::from_acl(py, acl))
                .transpose()
        };
        Py::new(
            py,
            PyMftAttributeX50 {
                revision: attr.revision,
                control: attr.control.to_string(),
                control_value: attr.control.bits(),
                owner: attr.owner.clone(),
                group: attr.group.clone(),
                sacl: acl(&attr.sacl)?,
                dacl: acl(&attr.dacl)?,
            },
        )
    }
}

/// An access control list.
#[pyclass(module = "mft")]
pub struct PyMftAcl {
    #[pyo3(get)]
    pub revision: u8,
    entries: Vec<Py<PyMftAce>>,
}

impl PyMftAcl {
    pub fn from_acl(py: Python, acl: &Acl) -> PyResult<Py<Self>> {
        let entries = acl
            .entries
            .iter()
            .map(|ace| PyMftAce::from_ace(py, ace))
            .collect::<PyResult<_>>()?;
        Py::new(
            py,
            PyMftAcl {
                revision: acl.revision,
                entries,
            },
        )
    }
}

#[pymethods]
impl PyMftAcl {
    pub fn entries(&self, py: Python) -> Vec<Py<PyMftAce>> {
        self.entries.iter().map(|ace| ace.clone_ref(py)).collect()
    }
}

/// An access control entry.
#[pyclass(module = "mft")]
pub struct PyMftAce {
    #[pyo3(get)]
    pub ace_type: u8,
    /// The name of a documented type (for example `ACCESS_ALLOWED`), without the `_ACE_TYPE`
    /// suffix.
    #[pyo3(get)]
    pub ace_type_name: Option<&'static str>,
    #[pyo3(get)]
    pub flags: String,
    /// `flags` as an integer.
    #[pyo3(get)]
    pub flags_value: u8,
    /// `None` for types whose layout isn't decoded.
    #[pyo3(get)]
    pub access_mask: Option<u32>,
    /// The object type of an object ACE, when its flags say it has one.
    #[pyo3(get)]
    pub object_type: Option<String>,
    /// The inherited object type of an object ACE, when its flags say it has one.
    #[pyo3(get)]
    pub inherited_object_type: Option<String>,
    #[pyo3(get)]
    pub sid: Option<String>,
    data: Vec<u8>,
}

impl PyMftAce {
    pub fn from_ace(py: Python, ace: &Ace) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAce {
                ace_type: ace.ace_type,
                ace_type_name: ace.ace_type_name,
                flags: ace.flags.to_string(),
                flags_value: ace.flags.bits(),
                access_mask: ace.access_mask,
                object_type: ace.object_type.as_ref().map(ToString::to_string),
                inherited_object_type: ace.inherited_object_type.as_ref().map(ToString::to_string),
                sid: ace.sid.clone(),
                data: ace.data.to_vec(),
            },
        )
    }
}

#[pymethods]
impl PyMftAce {
    /// The bytes after the decoded fields (for example a callback ACE's application data), or
    /// the whole body for types whose layout isn't decoded.
    #[getter]
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

#[pyclass(module = "mft")]
pub struct PyMftAttributeX80 {
    data: Vec<u8>,
}

impl PyMftAttributeX80 {
    pub fn from_data(py: Python, data: &[u8]) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeX80 {
                data: data.to_vec(),
            },
        )
    }
}

#[pymethods]
impl PyMftAttributeX80 {
    #[getter]
    pub fn data(&self) -> &[u8] {
        &self.data
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
    view_entries: Vec<Py<PyMftViewIndexEntry>>,
}

impl PyMftAttributeX90 {
    pub fn from_x90(py: Python, attr: &IndexRootAttr) -> PyResult<Py<Self>> {
        let index_entries = attr
            .index_entries
            .index_entries
            .iter()
            .map(|entry| PyMftIndexEntry::from_index_entry(py, entry))
            .collect::<PyResult<_>>()?;
        let view_entries = attr
            .index_entries
            .view_entries
            .iter()
            .map(|entry| PyMftViewIndexEntry::from_view_entry(py, entry))
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
                view_entries,
            },
        )
    }
}

#[pymethods]
impl PyMftAttributeX90 {
    /// The index entries stored in the index root itself, for a `$FILE_NAME` index (`$I30`).
    pub fn index_entries(&self, py: Python) -> Vec<Py<PyMftIndexEntry>> {
        self.index_entries
            .iter()
            .map(|entry| entry.clone_ref(py))
            .collect()
    }

    /// The index entries stored in the index root itself, for any other index (a view index
    /// such as `$Secure:$SII` or `$Extend\$Quota:$Q`).
    pub fn view_entries(&self, py: Python) -> Vec<Py<PyMftViewIndexEntry>> {
        self.view_entries
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
                flags: entry.flags.to_string(),
                fname_info: PyMftAttributeX30::from_x30(py, &entry.fname_info)?,
            },
        )
    }
}

/// An entry of a view index: one of the indexes NTFS keeps in its metadata files, recognized
/// from the index's collation rule and the sizes of the entry's key and data. `value_type` says
/// which of the optional fields are set:
/// - `SecurityDescriptor` (`$Secure:$SDH` or `$Secure:$SII`): `hash`, `security_id`,
///   `sds_offset` and `sds_length`
/// - `ObjectId` (`$Extend\$ObjId:$O`): `object_id`, `mft_reference_entry_id`,
///   `mft_reference_entry_sequence`, `birth_volume_id`, `birth_object_id` and `domain_id`
/// - `QuotaOwner` (`$Extend\$Quota:$O`): `sid` and `owner_id`
/// - `Quota` (`$Extend\$Quota:$Q`): `owner_id`, `version`, `quota_flags`, `bytes_used`,
///   `change_time`, `warning_limit` and `hard_limit`, and `exceeded_time` and `sid` when the
///   entry has them
/// - `Reparse` (`$Extend\$Reparse:$R`): `tag`, `tag_name`, `mft_reference_entry_id` and
///   `mft_reference_entry_sequence`
/// - `Other` (any other index): `key` and `data`
#[pyclass(module = "mft")]
#[derive(Default)]
pub struct PyMftViewIndexEntry {
    #[pyo3(get)]
    pub flags: String,
    /// `flags` as an integer.
    #[pyo3(get)]
    pub flags_value: u32,
    #[pyo3(get)]
    pub value_type: &'static str,
    #[pyo3(get)]
    pub hash: Option<u32>,
    #[pyo3(get)]
    pub security_id: Option<u32>,
    #[pyo3(get)]
    pub sds_offset: Option<u64>,
    #[pyo3(get)]
    pub sds_length: Option<u32>,
    #[pyo3(get)]
    pub object_id: Option<String>,
    #[pyo3(get)]
    pub mft_reference_entry_id: Option<u64>,
    #[pyo3(get)]
    pub mft_reference_entry_sequence: Option<u16>,
    #[pyo3(get)]
    pub birth_volume_id: Option<String>,
    #[pyo3(get)]
    pub birth_object_id: Option<String>,
    #[pyo3(get)]
    pub domain_id: Option<String>,
    #[pyo3(get)]
    pub sid: Option<String>,
    #[pyo3(get)]
    pub owner_id: Option<u32>,
    #[pyo3(get)]
    pub version: Option<u32>,
    #[pyo3(get)]
    pub quota_flags: Option<u32>,
    #[pyo3(get)]
    pub bytes_used: Option<u64>,
    change_time: Option<Timestamp>,
    /// -1 means no limit.
    #[pyo3(get)]
    pub warning_limit: Option<i64>,
    /// -1 means no limit.
    #[pyo3(get)]
    pub hard_limit: Option<i64>,
    exceeded_time: Option<Timestamp>,
    #[pyo3(get)]
    pub tag: Option<u32>,
    /// The name of a documented tag, as in `PyMftAttributeXC0.tag_name`.
    #[pyo3(get)]
    pub tag_name: Option<&'static str>,
    key: Option<Vec<u8>>,
    data: Option<Vec<u8>>,
}

impl PyMftViewIndexEntry {
    pub fn from_view_entry(py: Python, entry: &ViewIndexEntry) -> PyResult<Py<Self>> {
        let base = PyMftViewIndexEntry {
            flags: entry.flags.to_string(),
            flags_value: entry.flags.bits(),
            ..Default::default()
        };
        let view_entry = match &entry.value {
            ViewIndexValue::SecurityDescriptor {
                hash,
                security_id,
                sds_offset,
                sds_length,
            } => PyMftViewIndexEntry {
                value_type: "SecurityDescriptor",
                hash: Some(*hash),
                security_id: Some(*security_id),
                sds_offset: Some(*sds_offset),
                sds_length: Some(*sds_length),
                ..base
            },
            ViewIndexValue::ObjectId {
                object_id,
                mft_reference,
                birth_volume_id,
                birth_object_id,
                domain_id,
            } => PyMftViewIndexEntry {
                value_type: "ObjectId",
                object_id: Some(object_id.to_string()),
                mft_reference_entry_id: Some(mft_reference.entry),
                mft_reference_entry_sequence: Some(mft_reference.sequence),
                birth_volume_id: Some(birth_volume_id.to_string()),
                birth_object_id: Some(birth_object_id.to_string()),
                domain_id: Some(domain_id.to_string()),
                ..base
            },
            ViewIndexValue::QuotaOwner { sid, owner_id } => PyMftViewIndexEntry {
                value_type: "QuotaOwner",
                sid: Some(sid.clone()),
                owner_id: Some(*owner_id),
                ..base
            },
            ViewIndexValue::Quota {
                owner_id,
                version,
                quota_flags,
                bytes_used,
                change_time,
                warning_limit,
                hard_limit,
                exceeded_time,
                sid,
            } => PyMftViewIndexEntry {
                value_type: "Quota",
                owner_id: Some(*owner_id),
                version: Some(*version),
                quota_flags: Some(*quota_flags),
                bytes_used: Some(*bytes_used),
                change_time: Some(*change_time),
                warning_limit: Some(*warning_limit),
                hard_limit: Some(*hard_limit),
                exceeded_time: *exceeded_time,
                sid: sid.clone(),
                ..base
            },
            ViewIndexValue::Reparse {
                tag,
                tag_name,
                mft_reference,
            } => PyMftViewIndexEntry {
                value_type: "Reparse",
                tag: Some(*tag),
                tag_name: *tag_name,
                mft_reference_entry_id: Some(mft_reference.entry),
                mft_reference_entry_sequence: Some(mft_reference.sequence),
                ..base
            },
            ViewIndexValue::Other { key, data } => PyMftViewIndexEntry {
                value_type: "Other",
                key: Some(key.to_vec()),
                data: Some(data.to_vec()),
                ..base
            },
        };
        Py::new(py, view_entry)
    }
}

#[pymethods]
impl PyMftViewIndexEntry {
    /// When the quota entry last changed.
    #[getter]
    pub fn change_time(&self) -> PyResult<Option<Py<PyAny>>> {
        self.change_time.as_ref().map(date_to_pyobject).transpose()
    }

    /// When the owner went over their warning limit, if they did.
    #[getter]
    pub fn exceeded_time(&self) -> PyResult<Option<Py<PyAny>>> {
        self.exceeded_time
            .as_ref()
            .map(date_to_pyobject)
            .transpose()
    }

    /// The key of an `Other` entry.
    #[getter]
    pub fn key(&self) -> Option<&[u8]> {
        self.key.as_deref()
    }

    /// The data of an `Other` entry.
    #[getter]
    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }
}

/// The content of a non-resident attribute: its sizes, and the data runs that locate it on the
/// volume. The sizes are only valid in the attribute record whose `vcn_first` is 0.
#[pyclass(module = "mft")]
pub struct PyMftAttributeNonResident {
    inner: NonResidentAttr,
    /// The first virtual cluster number (VCN) this attribute record covers.
    #[pyo3(get)]
    pub vcn_first: u64,
    /// The last VCN this attribute record covers.
    #[pyo3(get)]
    pub vcn_last: u64,
    /// Allocated size in bytes, a multiple of the cluster size.
    #[pyo3(get)]
    pub allocated_length: u64,
    /// Real size in bytes.
    #[pyo3(get)]
    pub file_size: u64,
    /// Initialized size in bytes. Reads past it return zeroes.
    #[pyo3(get)]
    pub valid_data_length: u64,
    /// Compression unit size, as a power of two in clusters (0 if not compressed).
    #[pyo3(get)]
    pub unit_compression_size: u16,
    /// Bytes actually allocated on disk. Only read for compressed attributes, `None` otherwise.
    #[pyo3(get)]
    pub total_allocated: Option<u64>,
}

impl PyMftAttributeNonResident {
    pub fn from_non_resident(
        py: Python,
        header: &NonResidentHeader,
        attr: &NonResidentAttr,
    ) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeNonResident {
                inner: attr.clone(),
                vcn_first: header.vnc_first,
                vcn_last: header.vnc_last,
                allocated_length: header.allocated_length,
                file_size: header.file_size,
                valid_data_length: header.valid_data_length,
                unit_compression_size: header.unit_compression_size,
                total_allocated: header.total_allocated,
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

/// $VOLUME_NAME: the volume label.
#[pyclass(module = "mft")]
pub struct PyMftAttributeX60 {
    #[pyo3(get)]
    pub name: String,
}

impl PyMftAttributeX60 {
    pub fn from_x60(py: Python, attr: &VolumeNameAttr) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeX60 {
                name: attr.name.to_utf8_string(),
            },
        )
    }
}

/// $VOLUME_INFORMATION: the NTFS version and volume state flags.
#[pyclass(module = "mft")]
pub struct PyMftAttributeX70 {
    #[pyo3(get)]
    pub major_version: u8,
    #[pyo3(get)]
    pub minor_version: u8,
    #[pyo3(get)]
    pub flags: String,
    /// `flags` as an integer.
    #[pyo3(get)]
    pub flags_value: u16,
}

impl PyMftAttributeX70 {
    pub fn from_x70(py: Python, attr: &VolumeInformationAttr) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeX70 {
                major_version: attr.major_version,
                minor_version: attr.minor_version,
                flags: attr.flags.to_string(),
                flags_value: attr.flags.bits(),
            },
        )
    }
}

/// $REPARSE_POINT. `reparse_type` says which of the optional fields are set:
/// - `SymbolicLink`: `substitute_name`, `print_name` and `is_relative`
/// - `MountPoint` (a junction or volume mount point): `substitute_name` and `print_name`
/// - `AppExecLink`: `package_id`, `app_user_model_id` and `target_path`
/// - `LxSymlink` (a WSL symlink): `target`
/// - `Other` (any other tag, or data that doesn't match its tag): `data`
#[pyclass(module = "mft")]
#[derive(Default)]
pub struct PyMftAttributeXC0 {
    /// The reparse tag, which says which file system filter owns the data.
    #[pyo3(get)]
    pub tag: u32,
    /// The name of a documented tag, such as `SYMLINK`, without the `IO_REPARSE_TAG_` prefix.
    #[pyo3(get)]
    pub tag_name: Option<&'static str>,
    /// The owning filter's GUID, for third-party tags.
    #[pyo3(get)]
    pub guid: Option<String>,
    #[pyo3(get)]
    pub reparse_type: &'static str,
    #[pyo3(get)]
    pub substitute_name: Option<String>,
    #[pyo3(get)]
    pub print_name: Option<String>,
    #[pyo3(get)]
    pub is_relative: Option<bool>,
    #[pyo3(get)]
    pub package_id: Option<String>,
    #[pyo3(get)]
    pub app_user_model_id: Option<String>,
    #[pyo3(get)]
    pub target_path: Option<String>,
    #[pyo3(get)]
    pub target: Option<String>,
    data: Option<Vec<u8>>,
}

impl PyMftAttributeXC0 {
    pub fn from_xc0(py: Python, attr: &ReparsePointAttr) -> PyResult<Py<Self>> {
        let base = PyMftAttributeXC0 {
            tag: attr.tag,
            tag_name: attr.tag_name,
            guid: attr.guid.as_ref().map(ToString::to_string),
            ..Default::default()
        };
        let xc0 = match &attr.data {
            ReparseData::SymbolicLink {
                substitute_name,
                print_name,
                relative,
            } => PyMftAttributeXC0 {
                reparse_type: "SymbolicLink",
                substitute_name: Some(substitute_name.to_utf8_string()),
                print_name: Some(print_name.to_utf8_string()),
                is_relative: Some(*relative),
                ..base
            },
            ReparseData::MountPoint {
                substitute_name,
                print_name,
            } => PyMftAttributeXC0 {
                reparse_type: "MountPoint",
                substitute_name: Some(substitute_name.to_utf8_string()),
                print_name: Some(print_name.to_utf8_string()),
                ..base
            },
            ReparseData::AppExecLink {
                package_id,
                app_user_model_id,
                target_path,
            } => PyMftAttributeXC0 {
                reparse_type: "AppExecLink",
                package_id: Some(package_id.to_utf8_string()),
                app_user_model_id: Some(app_user_model_id.to_utf8_string()),
                target_path: Some(target_path.to_utf8_string()),
                ..base
            },
            ReparseData::LxSymlink { target } => PyMftAttributeXC0 {
                reparse_type: "LxSymlink",
                target: Some(target.to_string()),
                ..base
            },
            ReparseData::Other { data } => PyMftAttributeXC0 {
                reparse_type: "Other",
                data: Some(data.to_vec()),
                ..base
            },
        };
        Py::new(py, xc0)
    }
}

#[pymethods]
impl PyMftAttributeXC0 {
    /// The tag-specific data of an `Other` reparse point: the bytes after the reparse header
    /// (and the GUID of a third-party tag).
    #[getter]
    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }
}

/// $EA_INFORMATION: the sizes of the file's extended attributes in `$EA`.
#[pyclass(module = "mft")]
pub struct PyMftAttributeXD0 {
    /// The size of the extended attributes in packed form: each one's flags, lengths, name and
    /// value, without the next entry offsets and padding that `$EA` stores.
    #[pyo3(get)]
    pub packed_ea_size: u16,
    /// How many extended attributes have the `NEED_EA` flag.
    #[pyo3(get)]
    pub need_ea_count: u16,
    /// The size of the extended attributes as `$EA` stores them, which is the size of its value.
    #[pyo3(get)]
    pub unpacked_ea_size: u32,
}

impl PyMftAttributeXD0 {
    pub fn from_xd0(py: Python, attr: &EaInformationAttr) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeXD0 {
                packed_ea_size: attr.packed_ea_size,
                need_ea_count: attr.need_ea_count,
                unpacked_ea_size: attr.unpacked_ea_size,
            },
        )
    }
}

/// $EA: the file's extended attributes. Windows keeps its own metadata here too, such as WSL's
/// Linux owners and modes (`$LXUID`, `$LXGID`, `$LXMOD`, `$LXDEV`) and code integrity's
/// `$CI.CATALOGHINT`.
#[pyclass(module = "mft")]
pub struct PyMftAttributeXE0 {
    entries: Vec<Py<PyMftEaEntry>>,
}

impl PyMftAttributeXE0 {
    pub fn from_xe0(py: Python, attr: &EaAttr) -> PyResult<Py<Self>> {
        let entries = attr
            .entries
            .iter()
            .map(|entry| PyMftEaEntry::from_ea_entry(py, entry))
            .collect::<PyResult<_>>()?;
        Py::new(py, PyMftAttributeXE0 { entries })
    }
}

#[pymethods]
impl PyMftAttributeXE0 {
    pub fn entries(&self, py: Python) -> Vec<Py<PyMftEaEntry>> {
        self.entries
            .iter()
            .map(|entry| entry.clone_ref(py))
            .collect()
    }
}

/// One extended attribute. `value_type` says which of the optional fields are set:
/// - `LxInteger` (`$LXUID`, `$LXGID` or `$LXMOD`, a WSL file's Linux owner, group or mode):
///   `value`
/// - `LxDevice` (`$LXDEV`, the device numbers of a WSL device file): `major` and `minor`
/// - `CatalogHint` (`$CI.CATALOGHINT`, the security catalog that signs the file): `catalog`
/// - `Other` (any other value, or one that doesn't match its name's layout): `data`
#[pyclass(module = "mft")]
#[derive(Default)]
pub struct PyMftEaEntry {
    #[pyo3(get)]
    pub flags: String,
    /// `flags` as an integer.
    #[pyo3(get)]
    pub flags_value: u8,
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub value_type: &'static str,
    #[pyo3(get)]
    pub value: Option<u32>,
    #[pyo3(get)]
    pub major: Option<u32>,
    #[pyo3(get)]
    pub minor: Option<u32>,
    #[pyo3(get)]
    pub catalog: Option<String>,
    data: Option<Vec<u8>>,
}

impl PyMftEaEntry {
    pub fn from_ea_entry(py: Python, entry: &EaEntry) -> PyResult<Py<Self>> {
        let base = PyMftEaEntry {
            flags: entry.flags.to_string(),
            flags_value: entry.flags.bits(),
            name: entry.name.to_string(),
            ..Default::default()
        };
        let ea_entry = match &entry.value {
            EaValue::LxInteger { value } => PyMftEaEntry {
                value_type: "LxInteger",
                value: Some(*value),
                ..base
            },
            EaValue::LxDevice { major, minor } => PyMftEaEntry {
                value_type: "LxDevice",
                major: Some(*major),
                minor: Some(*minor),
                ..base
            },
            EaValue::CatalogHint { catalog } => PyMftEaEntry {
                value_type: "CatalogHint",
                catalog: Some(catalog.to_string()),
                ..base
            },
            EaValue::Other { data } => PyMftEaEntry {
                value_type: "Other",
                data: Some(data.to_vec()),
                ..base
            },
        };
        Py::new(py, ea_entry)
    }
}

#[pymethods]
impl PyMftEaEntry {
    /// The value of an `Other` entry.
    #[getter]
    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }
}

#[pyclass(module = "mft")]
pub struct PyMftAttributeOther {
    data: Vec<u8>,
}

impl PyMftAttributeOther {
    pub fn from_data(py: Python, data: &[u8]) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeOther {
                data: data.to_vec(),
            },
        )
    }
}

#[pymethods]
impl PyMftAttributeOther {
    #[getter]
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}
