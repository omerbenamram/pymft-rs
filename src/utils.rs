use pyo3::types::PyString;
use pyo3::{PyObject, PyResult, Python};
use pyo3_file::PyFileLikeObject;

#[derive(Debug)]
pub enum FileOrFileLike {
    File(String),
    FileLike(PyFileLikeObject),
}

impl FileOrFileLike {
    pub fn from_pyobject(path_or_file_like: PyObject) -> PyResult<FileOrFileLike> {
        let gil = Python::acquire_gil();
        let py = gil.python();

        // is a path
        if let Ok(string_ref) = path_or_file_like.cast_as::<PyString>(py) {
            return Ok(FileOrFileLike::File(
                string_ref.to_string_lossy().to_string(),
            ));
        }

        // We only need read + seek
        match PyFileLikeObject::with_requirements(path_or_file_like, true, false, true) {
            Ok(f) => Ok(FileOrFileLike::FileLike(f)),
            Err(e) => Err(e),
        }
    }
}
