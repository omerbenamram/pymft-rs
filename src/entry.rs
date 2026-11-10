use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex, PoisonError};

use crate::ReadSeek;
use pyo3::prelude::*;
use pyo3::types::PyString;

use crate::attribute::PyMftAttribute;
use crate::err::skip_or_raise;
use crate::utils::windows_path;
use mft_rs::attribute::header::MftAttributeHeader;
use mft_rs::attribute::{MftAttributeContent, MftAttributeType};
use mft_rs::{MftEntry, MftParser};

#[pyclass(module = "mft")]
pub struct PyMftEntry {
    // We need to keep inner entry to access it's attributes.
    inner: Arc<MftEntry>,
    /// Whether `attributes()` skips the attributes that fail to parse, as the parser does.
    skip_errors: bool,
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
    /// `flags` as an integer.
    #[pyo3(get)]
    pub flags_value: u16,
    #[pyo3(get)]
    pub is_allocated: bool,
    #[pyo3(get)]
    pub is_dir: bool,
    /// The $LogFile sequence number (LSN) of the last logged change to this record.
    #[pyo3(get)]
    pub lsn: u64,
    /// Whether the update sequence (fixup) values matched at the end of every sector. `None`
    /// when the record's signature isn't `FILE` (such as a `BAAD` record), so the fixups weren't
    /// checked.
    #[pyo3(get)]
    pub valid_fixup: Option<bool>,
    #[pyo3(get)]
    pub used_entry_size: u32,
    #[pyo3(get)]
    pub total_entry_size: u32,
    /// The path from the volume's root, with `\\` between names. `[UNKNOWN]` for a base record
    /// without a `$FILE_NAME` (where `entries_csv()` has an empty `FullPath`). A path can also
    /// start with a name that the mft crate gives: `[Orphaned]` when the parent directory is
    /// gone, `[Unknown]` when the parent's record isn't a directory.
    #[pyo3(get)]
    pub full_path: String,
    /// The size of the unnamed `$DATA` stream, or of a named one if there is none, also when an
    /// extension record holds it. 0 without a `$DATA` attribute. The same as `FileSize` in
    /// `entries_csv()`. Building the first entry whose `$ATTRIBUTE_LIST` is non-resident or fails
    /// to parse reads the whole MFT once, to find such files' other records.
    #[pyo3(get)]
    pub file_size: u64,
}

#[pymethods]
impl PyMftEntry {
    pub fn attributes(&self, py: Python) -> PyResult<Py<PyMftAttributesIter>> {
        // Only headers: content is parsed when `attribute_content` is read. A header that fails
        // to parse is kept as its error, which the iterator raises or skips.
        let mut attributes = Vec::new();
        for record in self.inner.iter_attribute_records_matching(None) {
            attributes.push(match record {
                Ok((header, _)) => Ok(PyMftAttribute::from_header(py, &self.inner, &header)?),
                // Boxed, as errors are rare and much bigger than an attribute's handle.
                Err(error) => Err(Box::new(error)),
            });
        }

        Py::new(
            py,
            PyMftAttributesIter {
                entry_id: self.entry_id,
                skip_errors: self.skip_errors,
                inner: Mutex::new(attributes.into_iter().enumerate()),
            },
        )
    }

    fn __repr__(&self, py: Python) -> PyResult<String> {
        Ok(format!(
            "<PyMftEntry entry_id={} sequence={} full_path={}>",
            self.entry_id,
            self.sequence,
            PyString::new(py, &self.full_path).repr()?
        ))
    }

    /// Entries are equal when they have the same number, record bytes and path, whichever
    /// parser or call they came from.
    fn __eq__(&self, other: PyRef<Self>) -> bool {
        self.entry_id == other.entry_id
            && self.full_path == other.full_path
            && self.inner.data == other.inner.data
    }

    fn __hash__(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        (self.entry_id, self.sequence, self.lsn).hash(&mut hasher);
        hasher.finish()
    }
}

impl PyMftEntry {
    pub fn from_mft_entry(
        py: Python,
        entry: MftEntry,
        parser: &mut MftParser<impl ReadSeek>,
        skip_errors: bool,
    ) -> PyResult<Py<PyMftEntry>> {
        let full_path = parser
            .get_full_path_for_entry(&entry)
            .expect("unreachable")
            .map_or_else(|| "[UNKNOWN]".to_string(), |path| windows_path(&path));

        let file_size = file_size(&entry, parser);

        Py::new(
            py,
            PyMftEntry {
                entry_id: entry.header.record_number,
                sequence: entry.header.sequence,
                base_entry_id: entry.header.base_reference.entry,
                base_entry_sequence: entry.header.base_reference.sequence,
                hard_link_count: entry.header.hard_link_count,
                flags: entry.header.flags.to_string(),
                flags_value: entry.header.flags.bits(),
                is_allocated: entry.is_allocated(),
                is_dir: entry.is_dir(),
                lsn: entry.header.metadata_transaction_journal,
                valid_fixup: entry.valid_fixup,
                used_entry_size: entry.header.used_entry_size,
                total_entry_size: entry.header.total_entry_size,
                inner: Arc::new(entry),
                skip_errors,
                full_path,
                file_size,
            },
        )
    }
}

/// Threads can share an iterator: each attribute goes to one of them.
#[pyclass(frozen, module = "mft")]
pub struct PyMftAttributesIter {
    entry_id: u64,
    skip_errors: bool,
    inner: Mutex<AttributesIter>,
}

type AttributesIter =
    std::iter::Enumerate<std::vec::IntoIter<Result<Py<PyMftAttribute>, Box<mft_rs::err::Error>>>>;

#[pymethods]
impl PyMftAttributesIter {
    fn __iter__(slf: PyRef<Self>) -> Py<PyMftAttributesIter> {
        slf.into()
    }

    fn __next__(&self, py: Python) -> PyResult<Option<Py<PyMftAttribute>>> {
        loop {
            // Only held to step, not while a warning runs Python code.
            let next = self
                .inner
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .next();
            match next {
                None => return Ok(None),
                Some((_, Ok(attribute))) => return Ok(Some(attribute)),
                Some((index, Err(error))) => skip_or_raise(
                    py,
                    self.skip_errors,
                    format_args!("attribute {index} of entry {}", self.entry_id),
                    *error,
                )?,
            }
        }
    }
}

/// A file's size as `entries_csv()` gives it (`FlatMftEntryWithName`): that of its unnamed `$DATA`
/// stream, or of a named one if there is none, among the `$DATA` attributes that parse. Records
/// that continue a stream started elsewhere don't hold its size.
///
/// A file with an `$ATTRIBUTE_LIST` can have its `$DATA` in an extension record. Those are only
/// read when the base record doesn't decide the size: it has no unnamed `$DATA`, and a list that
/// names a `$DATA` in another record, or doesn't say (a non-resident list is outside the MFT, and
/// one can fail to parse). For a list that doesn't say, the parser finds the records by reading
/// the whole MFT, once: it keeps what it found.
fn file_size(entry: &MftEntry, parser: &mut MftParser<impl ReadSeek>) -> u64 {
    /// A `$DATA` that starts a stream and parses, as (is named, size).
    fn stream(header: &MftAttributeHeader, record: &[u8]) -> Option<(bool, u64)> {
        (header.type_code == MftAttributeType::DATA
            && !header.is_continuation()
            && MftAttributeContent::from_record(record, header).is_ok())
        .then(|| (!header.name.is_empty(), header.value_size()))
    }

    let mut named_in_base = None;
    let mut data_may_be_elsewhere = false;
    for (header, record) in entry
        .iter_attribute_records_matching(Some(vec![
            MftAttributeType::AttributeList,
            MftAttributeType::DATA,
        ]))
        .filter_map(Result::ok)
    {
        if header.type_code == MftAttributeType::AttributeList {
            let content = MftAttributeContent::from_record(record, &header).ok();
            data_may_be_elsewhere |= content
                .as_ref()
                .and_then(MftAttributeContent::as_attribute_list)
                .is_none_or(|list| {
                    list.entries.iter().any(|listed| {
                        listed.attribute_type == MftAttributeType::DATA as u32
                            && listed.segment_reference.entry != entry.header.record_number
                    })
                });
        } else {
            match stream(&header, record) {
                Some((false, size)) => return size,
                Some((true, size)) => named_in_base = named_in_base.or(Some(size)),
                None => {}
            }
        }
    }
    if !data_may_be_elsewhere {
        return named_in_base.unwrap_or(0);
    }

    // The unnamed stream of an extension record, else the first named one, the base record's
    // first.
    let extensions = parser.get_extension_entries(entry);
    let mut named = named_in_base;
    for (header, record) in extensions
        .iter()
        .flat_map(|record| {
            record.iter_attribute_records_matching(Some(vec![MftAttributeType::DATA]))
        })
        .filter_map(Result::ok)
    {
        match stream(&header, record) {
            Some((false, size)) => return size,
            Some((true, size)) => named = named.or(Some(size)),
            None => {}
        }
    }
    named.unwrap_or(0)
}
