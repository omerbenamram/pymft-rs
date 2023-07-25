use log::{Level, Log, Metadata, Record, SetLoggerError};

use chrono::{DateTime, Datelike, Timelike, NaiveDateTime, Utc};
use pyo3::types::PyString;

#[cfg(not(feature = "abi3"))]
use pyo3::types::{PyDateTime, PyTzInfo};

use pyo3::ToPyObject;
use pyo3::{PyObject, PyResult, Python};
use pyo3_file::PyFileLikeObject;

use std::cmp::Ordering;

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

fn nanos_to_micros_round_half_even(nanos: u32) -> u32 {
    let nanos_e7 = (nanos % 1_000) / 100;
    let nanos_e6 = (nanos % 10_000) / 1000;
    let mut micros = (nanos / 10_000) * 10;
    match nanos_e7.cmp(&5) {
        Ordering::Greater => micros += nanos_e6 + 1,
        Ordering::Less => micros += nanos_e6,
        Ordering::Equal => micros += nanos_e6 + (nanos_e6 % 2),
    }
    micros
}

fn date_splitter(date: &DateTime<Utc>) -> PyResult<(i64, u32)> {
    let mut unix_time = date.timestamp();
    let mut micros = nanos_to_micros_round_half_even(date.timestamp_subsec_nanos());

    let inc_sec = micros / 1_000_000;
    micros %= 1_000_000;
    unix_time += inc_sec as i64;

    Ok((unix_time, micros))
}

#[cfg(not(feature = "abi3"))]
pub fn date_to_pyobject(date: &DateTime<Utc>) -> PyResult<PyObject> {
    // datetime has microsecond precision, so half-even round nanoseconds
    let (unix_time, micros) = date_splitter(date)?;
    let rounded_date = DateTime::<Utc>::from_utc(
        NaiveDateTime::from_timestamp_opt(unix_time, micros * 1_000)
            .expect("these values came from a DateTime, creating a new one from them cannot fail"),
        Utc
    );

    Python::with_gil(|py| {
        let utc = get_utc().ok();

        if utc.is_none() {
            log::warn!("UTC module not found, falling back to naive timezone objects")
        }

        let tz = utc.as_ref().map(|tz| tz.downcast::<PyTzInfo>(py).unwrap());

        PyDateTime::new(
            py,
            rounded_date.year(),
            rounded_date.month() as u8,
            rounded_date.day() as u8,
            rounded_date.hour() as u8,
            rounded_date.minute() as u8,
            rounded_date.second() as u8,
            rounded_date.timestamp_subsec_micros(),
            // Fallback to naive timestamps (None) if for some reason `datetime.timezone.utc` is not present.
            tz,
        )
        .map(|dt| dt.to_object(py))
    })
}

#[cfg(feature = "abi3")]
pub fn date_to_pyobject(date: &DateTime<Utc>) -> PyResult<PyObject> {
    // datetime has microsecond precision, so half-even round nanoseconds
    let (unix_time, micros) = date_splitter(date)?;
    let rounded_date = DateTime::<Utc>::from_utc(
        NaiveDateTime::from_timestamp(unix_time, micros * 1_000),
        Utc
    );

    // Create a UTC string in the format of `YYYY-MM-DDTHH:MM:SS.ssssssZ`
    // This is the format that the `datetime` module expects.
    // See: https://docs.python.org/3/library/datetime.html#datetime.datetime.strptime
    //

    use pyo3::types::PyDict;
    let utc_string = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}Z",
        rounded_date.year(),
        rounded_date.month(),
        rounded_date.day(),
        rounded_date.hour(),
        rounded_date.minute(),
        rounded_date.second(),
        rounded_date.timestamp_subsec_micros()
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
    // Replace this function with pyo3::types::timezone_utc when
    // Python < 3.2 is no longer supported.
    Python::with_gil(|py| {
        let datetime = py.import("datetime")?;
        let tz: PyObject = datetime.getattr("timezone")?.into();
        let utc = tz.getattr(py, "utc")?;

        Ok(utc)
    })
}

#[cfg(test)]
mod tests {
    use pyo3::types::{PyDateAccess, PyTimeAccess};

    use super::*;

    #[test]
    fn test_nanos_to_micros_round_half_even() {
        assert_eq!(nanos_to_micros_round_half_even(764_026_300), 764_026);
        assert_eq!(nanos_to_micros_round_half_even(764_026_600), 764_027);
        assert_eq!(nanos_to_micros_round_half_even(764_026_500), 764_026);
        assert_eq!(nanos_to_micros_round_half_even(764_027_500), 764_028);
        assert_eq!(nanos_to_micros_round_half_even(999_999_500), 1_000_000);
    }

    #[test]
    fn test_date_splitter(){
        let tests = [
            ("2020-09-29T17:38:04.9999995Z", (1601401085, 0u32)),
            ("2020-09-29T17:38:04.0000004Z", (1601401084, 0u32)),
            ("2020-09-29T17:38:04.1234567Z", (1601401084, 123457u32)),
            ("2020-12-31T23:59:59.9999995Z", (1609459200, 0u32)),
        ];

        for (test, expected) in tests {
            let dt = DateTime::parse_from_rfc3339(test).unwrap().with_timezone(&Utc);
            let res = date_splitter(&dt).unwrap();
            assert_eq!(res, expected);
        }
    }

    #[test]
    fn test_date_to_pyobject() {
        let tests = [
            ("2020-09-29T17:38:04.9999995Z", (2020, 9, 29, 17, 38, 5, 0)),
            ("2020-09-29T17:38:04.0000004Z", (2020, 9, 29, 17, 38, 4, 0)),
            ("2020-09-29T17:38:04.1234567Z", (2020, 9, 29, 17, 38, 4, 123457)),
            ("2020-12-31T23:59:59.9999995Z", (2021, 1, 1, 0, 0, 0, 0)),
        ];
        Python::with_gil(|py| {
            for (test, (y, mo, d, h, min, s, us)) in tests {
                let dt = DateTime::parse_from_rfc3339(test).unwrap().with_timezone(&Utc);

                let po = date_to_pyobject(&dt).unwrap();
                let pdt = po.as_ref(py).extract::<&PyDateTime>().unwrap();

                assert_eq!(pdt.get_year(), y);
                assert_eq!(pdt.get_month(), mo);
                assert_eq!(pdt.get_day(), d);
                assert_eq!(pdt.get_hour(), h);
                assert_eq!(pdt.get_minute(), min);
                assert_eq!(pdt.get_second(), s);
                assert_eq!(pdt.get_microsecond(), us);
            }
        });
    }
}
