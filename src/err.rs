use pyo3::PyErr;
use pyo3::exceptions::RuntimeError;

pub struct PyMftError(pub mft::err::Error);

impl From<PyMftError> for PyErr {
    fn from(err: PyMftError) -> Self {
        match err.0 {
            mft::err::Error::IoError {
                source,
                backtrace: _,
            } => source.into(),
            _ => PyErr::new::<RuntimeError, _>(format!("{}", err.0)),
        }
    }
}
