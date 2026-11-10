use std::fmt;

use pyo3::exceptions::PyRuntimeWarning;
use pyo3::prelude::*;
use pyo3::{PyErr, exceptions};

use crate::utils::warn;

pub struct PyMftError(pub mft_rs::err::Error);

impl From<PyMftError> for PyErr {
    fn from(err: PyMftError) -> Self {
        match err.0 {
            mft_rs::err::Error::IoError { source } => source.into(),
            _ => PyErr::new::<exceptions::PyRuntimeError, _>(format!("{}", err.0)),
        }
    }
}

/// Raises `error`, which a record failed to parse with, or with `skip_errors` warns that `what` is
/// skipped because of it. If a `warnings` filter turns the warning into an error, that is raised,
/// caused by `error`.
pub(crate) fn skip_or_raise(
    py: Python,
    skip_errors: bool,
    what: fmt::Arguments,
    error: mft_rs::err::Error,
) -> PyResult<()> {
    let error = PyErr::from(PyMftError(error));
    if !skip_errors {
        return Err(error);
    }
    let message = format!("Skipping {what}: {error}");
    warn(py, &py.get_type::<PyRuntimeWarning>(), message)
        .inspect_err(|warning| warning.set_cause(py, Some(error)))
}
