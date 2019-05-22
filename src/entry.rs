use crate::ReadSeek;
use pyo3::prelude::*;

use mft::{MftEntry, MftParser};
use pyo3::{Py, PyResult, Python};
use std::path::PathBuf;

#[pyclass]
pub struct PyMftEntry {
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
                entry_id: entry.header.record_number,
                sequence: entry.header.sequence,
                base_entry_id: entry.header.base_reference.entry,
                base_entry_sequence: 0,
                hard_link_count: 0,
                flags: format!("{:?}", entry.header.flags),
                used_entry_size: entry.header.used_entry_size,
                total_entry_size: entry.header.total_entry_size,
                full_path,
            },
        )
    }
}
