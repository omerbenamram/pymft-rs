use mft_rs::attribute::x90::{IndexRootAttr, IndexRootFlags, IndexEntryHeader, IndexEntries};
use pyo3::prelude::*;
use pyo3::{PyResult, Python, ToPyObject};
use crate::attribute::PyMftAttributeX30;

#[pyclass]
#[derive(Clone)]
pub struct PyMftAttributeX90IndexEntryHeader {
    #[pyo3(get)]
    mft_reference_entry_id: u64,
    #[pyo3(get)]
    mft_reference_entry_sequence: u16,
    #[pyo3(get)]
    pub index_record_length: u16,
    #[pyo3(get)]
    pub attr_fname_length: u16,
    #[pyo3(get)]
    pub fname_info: PyMftAttributeX30,
}

impl PyMftAttributeX90IndexEntryHeader {
    pub fn from_index_entry_header(py: Python, index_entry_header: &IndexEntryHeader) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeX90IndexEntryHeader {
                mft_reference_entry_id: index_entry_header.mft_reference.entry,
                mft_reference_entry_sequence: index_entry_header.mft_reference.sequence,
                index_record_length: index_entry_header.index_record_length,
                attr_fname_length: index_entry_header.attr_fname_length,
                fname_info: PyMftAttributeX30::from_x30(py, index_entry_header.fname_info.clone())?.to_object(py).extract(py)?
            },
        )
    }
}

#[pyclass]
#[derive(Clone)]
pub struct PyMftAttributeIndexRoot {
    inner: IndexEntries,
}

#[pyclass]
pub struct PyMftAttributeIndexRootIndexEntriesIter {
    inner: Box<dyn Iterator<Item = PyObject> + Send>
}

#[pymethods]
impl PyMftAttributeIndexRootIndexEntriesIter {
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

impl PyMftAttributeIndexRoot {
    pub fn from_index_entry(py: Python, attr: IndexEntries) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeIndexRoot {
                inner: attr
            },
        )
    }
}

#[pymethods]
impl PyMftAttributeIndexRoot {
    pub fn index_entries(&self) -> PyResult<Py<PyMftAttributeIndexRootIndexEntriesIter>> {
        Python::with_gil(|py| {
            let mut index_entries = vec![];

            for index_entry in &self.inner.index_entries {
                match PyMftAttributeX90IndexEntryHeader::from_index_entry_header(py, index_entry).map(|index_entry| index_entry.to_object(py)) {
                    Ok(obj) => { index_entries.push(obj) }
                    Err(e) => { index_entries.push(e.to_object(py)) }
                }
            }

            Py::new(
                py,
                PyMftAttributeIndexRootIndexEntriesIter {
                    inner: Box::new(index_entries.into_iter())
                },
            )
        })
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
    pub collation_rule: String,
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
    pub is_large_index: bool,
    #[pyo3(get)]
    pub index_entries: PyMftAttributeIndexRoot
}

impl PyMftAttributeX90 {
    pub fn from_x90(py: Python, attr: IndexRootAttr) -> PyResult<Py<Self>> {
        Py::new(
            py,
            PyMftAttributeX90 {
                attribute_type: attr.attribute_type,
                collation_rule: format!("{:?}", attr.collation_rule),
                index_entry_size: attr.index_entry_size,
                index_entry_number_of_cluster_blocks: attr.index_entry_number_of_cluster_blocks,
                relative_offset_to_index_node: attr.relative_offset_to_index_node,
                index_node_length: attr.index_node_length,
                index_node_allocation_length: attr.index_node_allocation_length,
                is_large_index: attr.index_root_flags == IndexRootFlags::LARGE_INDEX,
                index_entries: PyMftAttributeIndexRoot::from_index_entry(py, attr.index_entries)?.to_object(py).extract(py)?
            },
        )
    }
}
