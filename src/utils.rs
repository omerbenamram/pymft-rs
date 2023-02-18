use log::{Level, Log, Metadata, Record, SetLoggerError};

use chrono::{DateTime, Datelike, Timelike, Utc};
use pyo3::types::PyString;

#[cfg(not(feature = "abi3"))]
use pyo3::types::{PyDateTime, PyTzInfo};

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

#[cfg(not(feature = "abi3"))]
pub fn date_to_pyobject(date: &DateTime<Utc>) -> PyResult<PyObject> {
    Python::with_gil(|py| {
        let utc = get_utc().ok();

        if utc.is_none() {
            log::warn!("UTC module not found, falling back to naive timezone objects")
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

#[cfg(feature = "abi3")]
pub fn date_to_pyobject(date: &DateTime<Utc>) -> PyResult<PyObject> {
    // Create a UTC string in the format of `YYYY-MM-DDTHH:MM:SS.ssssssZ`
    // This is the format that the `datetime` module expects.
    // See: https://docs.python.org/3/library/datetime.html#datetime.datetime.strptime
    //

    use pyo3::types::PyDict;
    let utc_string = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}Z",
        date.year(),
        date.month(),
        date.day(),
        date.hour(),
        date.minute(),
        date.second(),
        date.timestamp_subsec_micros()
    );

    Python::with_gil(|py| {
        let datetime = py.import("datetime")?;
        let datetime: PyObject = datetime.getattr("datetime")?.into();
        let datetime = datetime.getattr(py, "strptime")?;

        let obj = datetime.call1(py, (utc_string, "%Y-%m-%dT%H:%M:%S.%fZ"))?;
        // call replace on the datetime object to replace the tzinfo with the UTC tzinfo

        let kwargs: &PyDict = PyDict::from_sequence(
            py,
            [("tzinfo".to_object(py), get_utc()?.to_object(py))].to_object(py),
        )
        .expect("we have GIL");

        obj.call_method(py, "replace", (), Some(kwargs))
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
