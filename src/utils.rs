use log::{Level, Log, Metadata, Record, SetLoggerError};

use jiff::tz::Offset;
use mft_rs::Timestamp;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyAnyMethods, PyString, PyStringMethods};
use pyo3_file::PyFileLikeObject;
use std::path::Path;

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
                    .cast::<PyString>()
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

/// `datetime.datetime` and `datetime.timezone.utc`, looked up once per interpreter.
static DATETIME_TYPE_AND_UTC: PyOnceLock<(Py<PyAny>, Py<PyAny>)> = PyOnceLock::new();

/// Converts a timestamp to a timezone-aware (UTC) `datetime.datetime`.
///
/// This is called for every timestamp getter, so the `datetime` module lookups are cached and the
/// calendar fields are computed in Rust: a single Python call per timestamp, instead of importing
/// the module and building `epoch + timedelta(...)` every time.
pub fn date_to_pyobject(date: &Timestamp) -> PyResult<Py<PyAny>> {
    Python::attach(|py| {
        let (datetime, utc) = DATETIME_TYPE_AND_UTC.get_or_try_init(py, || -> PyResult<_> {
            let datetime_mod = py.import("datetime")?;
            let datetime = datetime_mod.getattr("datetime")?.unbind();
            let utc = datetime_mod.getattr("timezone")?.getattr("utc")?.unbind();
            Ok((datetime, utc))
        })?;

        // Truncate to whole microseconds first (toward zero, like `as_microsecond`), so values
        // before 1970 round the same way the previous `epoch + timedelta(microseconds=...)` did.
        let micros = Timestamp::from_microsecond(date.as_microsecond())
            .expect("truncating toward the epoch stays in range");
        let dt = Offset::UTC.to_datetime(micros);
        let dt = datetime.bind(py).call1((
            dt.year(),
            dt.month(),
            dt.day(),
            dt.hour(),
            dt.minute(),
            dt.second(),
            dt.subsec_nanosecond() / 1_000,
            utc.bind(py),
        ))?;
        Ok(dt.unbind())
    })
}

/// Joins a resolved path with `\`, the way Windows shows NTFS paths, whatever the host platform.
pub fn windows_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("\\")
}
