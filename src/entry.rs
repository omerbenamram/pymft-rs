use std::sync::Arc;

use crate::ReadSeek;
use pyo3::prelude::*;

use crate::attribute::PyMftAttribute;
use crate::err::skip_or_raise;
use crate::utils::windows_path;
use mft_rs::attribute::MftAttributeType;
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
    #[pyo3(get)]
    pub full_path: String,
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
                inner: attributes.into_iter().enumerate(),
            },
        )
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

        let file_size = entry
            .iter_attribute_records_matching(Some(vec![MftAttributeType::DATA]))
            .find_map(Result::ok)
            .map_or(0, |(header, _)| header.value_size());

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

#[pyclass(module = "mft")]
pub struct PyMftAttributesIter {
    entry_id: u64,
    skip_errors: bool,
    inner: std::iter::Enumerate<
        std::vec::IntoIter<Result<Py<PyMftAttribute>, Box<mft_rs::err::Error>>>,
    >,
}

#[pymethods]
impl PyMftAttributesIter {
    fn __iter__(slf: PyRefMut<Self>) -> PyResult<Py<PyMftAttributesIter>> {
        Ok(slf.into())
    }

    fn __next__(&mut self, py: Python) -> PyResult<Option<Py<PyMftAttribute>>> {
        for (index, attribute) in self.inner.by_ref() {
            match attribute {
                Ok(attribute) => return Ok(Some(attribute)),
                Err(error) => skip_or_raise(
                    py,
                    self.skip_errors,
                    format_args!("attribute {index} of entry {}", self.entry_id),
                    *error,
                )?,
            }
        }
        Ok(None)
    }
}
