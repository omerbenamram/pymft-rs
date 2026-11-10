use log::{Level, Log, Metadata, Record, SetLoggerError};

use jiff::tz::Offset;
use mft_rs::Timestamp;
use pyo3::exceptions::{PyTypeError, PyUserWarning};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyAnyMethods, PyBytes, PyString, PyTypeMethods};
use pyo3_file::PyFileLikeObject;
use std::cell::RefCell;
use std::ffi::CString;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum FileOrFileLike {
    File(PathBuf),
    FileLike(PinnedFileLike),
}

/// A Python file object, read at a position kept here instead of the object's own.
///
/// The object stays usable from Python, where anything can move its position: the caller's own
/// `seek` or `read`, or a second parser over the same object. So every read seeks first. A reader
/// that trusts the stream to stay where its last read left it (as `MftParser` does for
/// sequential entries) would return the bytes of another entry.
#[derive(Debug)]
pub struct PinnedFileLike {
    inner: PyFileLikeObject,
    position: u64,
}

impl PinnedFileLike {
    fn new(inner: PyFileLikeObject) -> Self {
        PinnedFileLike { inner, position: 0 }
    }
}

impl Read for PinnedFileLike {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = Python::attach(|py| {
            self.inner.py_seek(py, SeekFrom::Start(self.position))?;
            self.inner.py_read(py, buf)
        })?;
        self.position += read as u64;
        Ok(read)
    }
}

impl Seek for PinnedFileLike {
    /// Only moves the position kept here, which the next read seeks to. Seeking from the end asks
    /// the object, as that is the only way to learn its size.
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.position = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::Current(delta) => self
                .position
                .checked_add_signed(delta)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek out of range"))?,
            SeekFrom::End(_) => self.inner.seek(pos)?,
        };
        Ok(self.position)
    }
}

impl FileOrFileLike {
    pub fn from_pyobject(path_or_file_like: &Bound<'_, PyAny>) -> PyResult<FileOrFileLike> {
        let py = path_or_file_like.py();

        // A file object first: one can have a path too (fsspec's `LocalFileOpener`, a wrapper
        // that decompresses), and what its `read` gives isn't what is on disk under that name.
        // `read` and `seek` are all that is used, so `PyFileLikeObject` has nothing more to check.
        if path_or_file_like.hasattr("read")? && path_or_file_like.hasattr("seek")? {
            // A text file would be read as its characters encoded back to UTF-8.
            let text_io = py.import("io")?.getattr("TextIOBase")?;
            if path_or_file_like.is_instance(&text_io)? {
                return Err(PyTypeError::new_err(
                    "expected a file opened in binary mode ('rb'), not a text file",
                ));
            }
            return PyFileLikeObject::py_new(path_or_file_like.clone())
                .map(|file| FileOrFileLike::FileLike(PinnedFileLike::new(file)));
        }

        // A path: `str`, `bytes` or `os.PathLike`. `os.fsdecode` takes the three and decodes
        // bytes the way Python does file names, so one that isn't UTF-8 is kept as it is.
        if path_or_file_like.is_instance_of::<PyString>()
            || path_or_file_like.is_instance_of::<PyBytes>()
            || path_or_file_like.hasattr("__fspath__")?
        {
            let path = py
                .import("os")?
                .call_method1("fsdecode", (path_or_file_like,))?;
            return Ok(FileOrFileLike::File(path.extract()?));
        }

        Err(PyTypeError::new_err(format!(
            "expected a path (str, bytes or os.PathLike) or a binary file-like object with \
             read() and seek(), not {}",
            path_or_file_like.get_type().name()?
        )))
    }
}

thread_local! {
    /// The warnings logged on this thread inside [`with_deferred_warnings`], if it is running.
    static DEFERRED_WARNINGS: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

/// Puts back the outer call's warnings, also when unwinding.
struct DeferredWarningsScope(Option<Vec<String>>);

impl Drop for DeferredWarningsScope {
    fn drop(&mut self) {
        DEFERRED_WARNINGS.set(self.0.take());
    }
}

/// Runs `f`, which locks the parser, and only then sends the warnings the `mft` crate logged in
/// it to `warnings.warn`, which runs Python code: a `warnings` hook that uses the parser would
/// wait for the lock forever. If a `warnings` filter turns one into an error, it is raised in
/// place of the value of `f`. An error of `f` comes first.
pub fn with_deferred_warnings<T>(py: Python<'_>, f: impl FnOnce() -> PyResult<T>) -> PyResult<T> {
    let (result, messages) = {
        // Calls nest when a file-like object's `read` uses another parser.
        let _scope = DeferredWarningsScope(DEFERRED_WARNINGS.replace(Some(Vec::new())));
        let result = f();
        (result, DEFERRED_WARNINGS.take().unwrap_or_default())
    };
    let category = py.get_type::<PyUserWarning>();
    match result {
        Ok(value) => {
            for message in messages {
                warn(py, &category, message)?;
            }
            Ok(value)
        }
        Err(error) => {
            // Still shown, but only the error of `f` is raised.
            for message in messages {
                if warn(py, &category, message).is_err() {
                    break;
                }
            }
            Err(error)
        }
    }
}

/// `warnings.warn(message, category)`, which raises if a `warnings` filter says so.
pub fn warn(py: Python<'_>, category: &Bound<'_, PyAny>, message: String) -> PyResult<()> {
    // A warning's message can't hold a NUL.
    let message = CString::new(message.replace('\0', "\\0")).expect("NULs were escaped");
    PyErr::warn(py, category, &message, 1)
}

/// Sends the warnings the `mft` crate logs to Python's `warnings`.
struct PyLogger {
    level: Level,
}

impl Log for PyLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let message = format!(
            "{:<5} [{}] {}",
            record.level(),
            record.module_path().unwrap_or_default(),
            record.args()
        );
        let message = DEFERRED_WARNINGS.with_borrow_mut(|deferred| match deferred {
            Some(messages) => {
                messages.push(message);
                None
            }
            None => Some(message),
        });

        // Logged outside `with_deferred_warnings`, so no lock is held, and nothing can raise an
        // error from here either.
        if let Some(message) = message {
            Python::attach(|py| {
                if let Err(error) = warn(py, &py.get_type::<PyUserWarning>(), message) {
                    error.write_unraisable(py, None);
                }
            })
        }
    }

    fn flush(&self) {}
}

pub fn init_logging() -> Result<(), SetLoggerError> {
    log::set_boxed_logger(Box::new(PyLogger { level: Level::Warn }))?;
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
