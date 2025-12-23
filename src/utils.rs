use log::{Level, Log, Metadata, Record, SetLoggerError};

use mft_rs::Timestamp;
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyString, PyStringMethods};
use pyo3_file::PyFileLikeObject;

#[derive(Debug)]
pub enum FileOrFileLike {
    File(String),
    FileLike(PyFileLikeObject),
}

impl FileOrFileLike {
    pub fn from_pyobject(path_or_file_like: Py<PyAny>) -> PyResult<FileOrFileLike> {
        Python::attach(|py| {
            // Is a path
            let maybe_path: Option<String> = {
                let bound = path_or_file_like.bind(py);
                bound
                    .downcast::<PyString>()
                    .ok()
                    .map(|s| s.to_string_lossy().into_owned())
            };

            if let Some(path) = maybe_path {
                return Ok(FileOrFileLike::File(path));
            }

            // We only need read + seek (no write, no fileno).
            PyFileLikeObject::with_requirements(path_or_file_like, true, false, true, false)
                .map(FileOrFileLike::FileLike)
        })
    }
}

/// A logger that prints all messages with a readable output format.
struct PyLogger {
    level: Level,
    warnings_module: Py<PyAny>,
}

impl Log for PyLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        if let Level::Warn = self.level {
            let level_string = record.level().to_string();
            Python::attach(|py| {
                let message = format!(
                    "{:<5} [{}] {}",
                    level_string,
                    record.module_path().unwrap_or_default(),
                    record.args()
                );

                let _ = self
                    .warnings_module
                    .bind(py)
                    .call_method1("warn", (message,));
            })
        }
    }

    fn flush(&self) {}
}

pub fn init_logging(py: Python<'_>) -> Result<(), SetLoggerError> {
    let warnings = py
        .import("warnings")
        .expect("python to have warnings module")
        .into_any()
        .unbind();

    let logger = PyLogger {
        level: Level::Warn,
        warnings_module: warnings,
    };

    log::set_boxed_logger(Box::new(logger))?;
    log::set_max_level(Level::Warn.to_level_filter());

    Ok(())
}

pub fn date_to_pyobject(date: &Timestamp) -> PyResult<Py<PyAny>> {
    let micros = date.as_microsecond();

    Python::attach(|py| {
        let datetime_mod = py.import("datetime")?;

        let datetime = datetime_mod.getattr("datetime")?;
        let timezone = datetime_mod.getattr("timezone")?;
        let utc = timezone.getattr("utc")?;
        let epoch = datetime.call1((1970, 1, 1, 0, 0, 0, 0, utc))?;

        let timedelta = datetime_mod.getattr("timedelta")?;
        let delta = timedelta.call1((0, 0, micros))?;

        let dt = epoch.call_method1("__add__", (delta,))?;
        Ok(dt.unbind())
    })
}
