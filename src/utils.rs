use log::{Level, Log, Metadata, Record, SetLoggerError};

use chrono::{DateTime, Utc};
use pyo3::types::{PyDateTime, PyString, PyTzInfo};
use pyo3::{ToPyObject, Py};
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

/// A logger that prints all messages with a readable output format.
struct PyLogger {
    level: Level,
    warnings_module: PyObject,
}

impl Log for PyLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            if let Level::Warn = self.level {
                let level_string = record.level().to_string();
                let gil = Python::acquire_gil();
                let py = gil.python();

                let message = format!(
                    "{:<5} [{}] {}",
                    level_string,
                    record.module_path().unwrap_or_default(),
                    record.args()
                );

                self.warnings_module
                    .call_method(py, "warn", (message,), None)
                    .ok();
            }
        }
    }

    fn flush(&self) {}
}

pub fn init_logging(py: Python) -> Result<(), SetLoggerError> {
    let warnings = py
        .import("warnings")
        .expect("python to have warning module")
        .to_object(py);

    let logger = PyLogger {
        level: Level::Warn,
        warnings_module: warnings,
    };

    log::set_boxed_logger(Box::new(logger))?;
    log::set_max_level(Level::Warn.to_level_filter());

    Ok(())
}

pub fn date_to_pyobject(date: &DateTime<Utc>) -> PyResult<Py<PyDateTime>> {
    let gil = Python::acquire_gil();
    let py = gil.python();

    PyDateTime::from_timestamp(
        py,
        date.timestamp() as f64,
        // Fallback to naive timestamps (None) if for some reason `datetime.timezone.utc` is not present.
        get_utc()
            .ok()
            .as_ref()
            .and_then(|u| Some(u.cast_as::<PyTzInfo>(py).expect("utc to be a PyTzInfo"))),
    )
}

pub fn get_utc() -> PyResult<PyObject> {
    let gil = Python::acquire_gil();
    let py = gil.python();

    let datetime = py.import("datetime")?;
    let tz: PyObject = datetime.get("timezone")?.into();
    let utc = tz.getattr(py, "utc")?;

    Ok(utc)
}
