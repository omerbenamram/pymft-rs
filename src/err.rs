use pyo3::{exceptions, PyErr};

pub struct PyMftError(pub mft_rs::err::Error);

impl From<PyMftError> for PyErr {
    fn from(err: PyMftError) -> Self {
        match err.0 {
            mft_rs::err::Error::IoError { source } => source.into(),
            _ => PyErr::new::<exceptions::PyRuntimeError, _>(format!("{}", err.0)),
        }
    }
}
