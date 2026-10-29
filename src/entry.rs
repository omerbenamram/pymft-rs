use std::sync::Arc;

use crate::ReadSeek;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::*;

use crate::attribute::PyMftAttribute;
use crate::err::PyMftError;
use crate::utils::windows_path;
use mft_rs::attribute::MftAttributeType;
use mft_rs::{MftEntry, MftParser};

#[pyclass(module = "mft")]
pub struct PyMftEntry {
    // We need to keep inner entry to access it's attributes.
    inner: Arc<MftEntry>,
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
    pub fn attributes(&self) -> PyResult<Py<PyMftAttributesIter>> {
        Python::attach(|py| {
            let mut attributes = vec![];

            // Only headers: content is parsed when `attribute_content` is read.
            for record in self.inner.iter_attribute_records_matching(None) {
                match record {
                    Ok((header, _)) => {
                        match PyMftAttribute::from_header(py, &self.inner, &header)
                            .and_then(|entry| entry.into_py_any(py))
                        {
                            Ok(obj) => attributes.push(obj),
                            Err(e) => attributes.push(e.into_py_any(py).unwrap()),
                        }
                    }
                    Err(e) => attributes.push(PyErr::from(PyMftError(e)).into_py_any(py).unwrap()),
                }
            }

            Py::new(
                py,
                PyMftAttributesIter {
                    inner: Box::new(attributes.into_iter()),
                },
            )
        })
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
                full_path,
                file_size,
            },
        )
    }
}

#[pyclass(module = "mft")]
pub struct PyMftAttributesIter {
    inner: Box<dyn Iterator<Item = Py<PyAny>> + Send + Sync>,
}

#[pymethods]
impl PyMftAttributesIter {
    fn __iter__(slf: PyRefMut<Self>) -> PyResult<Py<PyMftAttributesIter>> {
        Ok(slf.into())
    }

    fn __next__(mut slf: PyRefMut<Self>) -> PyResult<Option<Py<PyAny>>> {
        slf.next()
    }
}

impl PyMftAttributesIter {
    fn next(&mut self) -> PyResult<Option<Py<PyAny>>> {
        // Extract the result out of the iterator, so iteration will return error, but can continue.
        Ok(self.inner.next())
    }
}
