use log::{Level, Log, Metadata, Record, SetLoggerError};

use chrono::{DateTime, Datelike, Timelike, Utc};
use log::warn;
use pyo3::types::{PyDateTime, PyString, PyTzInfo};
use pyo3::ToPyObject;
use pyo3::{PyObject, PyResult, Python};
use pyo3_file::PyFileLikeObject;

#[derive(Debug)]
pub enum FileOrFileLike {
    File(String),
    FileLike(PyFileLikeObject),
}

impl FileOrFileLike {
    pub fn from_pyobject(path_or_file_like: PyObject) -> PyResult<FileOrFileLike> {
        Python::with_gil(|py| {
            // is a path
            if let Ok(string_ref) = path_or_file_like.downcast::<PyString>(py) {
                return Ok(FileOrFileLike::File(
                    string_ref.to_string_lossy().to_string(),
                ));
            }

            // We only need read + seek
            match PyFileLikeObject::with_requirements(path_or_file_like, true, false, true) {
                Ok(f) => Ok(FileOrFileLike::FileLike(f)),
                Err(e) => Err(e),
            }
        })
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
                Python::with_gil(|py| {
                    let message = format!(
                        "{:<5} [{}] {}",
                        level_string,
                        record.module_path().unwrap_or_default(),
                        record.args()
                    );

                    self.warnings_module
                        .call_method(py, "warn", (message,), None)
                        .ok();
                })
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

pub fn date_to_pyobject(date: &DateTime<Utc>) -> PyResult<PyObject> {
    Python::with_gil(|py| {
        let utc = get_utc().ok();

        if utc.is_none() {
            warn!("UTC module not found, falling back to naive timezone objects")
        }

        let tz = utc.as_ref().map(|tz| tz.downcast::<PyTzInfo>(py).unwrap());

        PyDateTime::new(
            py,
            date.year(),
            date.month() as u8,
            date.day() as u8,
            date.hour() as u8,
            date.minute() as u8,
            date.second() as u8,
            date.timestamp_subsec_micros(),
            // Fallback to naive timestamps (None) if for some reason `datetime.timezone.utc` is not present.
            tz,
        )
        .map(|dt| dt.to_object(py))
    })
}

pub fn get_utc() -> PyResult<PyObject> {
    Python::with_gil(|py| {
        let datetime = py.import("datetime")?;
        let tz: PyObject = datetime.getattr("timezone")?.into();
        let utc = tz.getattr(py, "utc")?;

        Ok(utc)
    })
}
