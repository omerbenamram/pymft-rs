use log::{Level, Log, Metadata, Record, SetLoggerError};

use jiff::tz::Offset;
use mft_rs::Timestamp;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyAnyMethods, PyBytes, PyString, PyTypeMethods};
use pyo3_file::PyFileLikeObject;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum FileOrFileLike {
    File(PathBuf),
    FileLike(PinnedFileLike),
}

/// How much of a Python file object is fetched at a time.
const WINDOW_SIZE: usize = 64 * 1024;

/// A Python file object, read at a position kept here instead of the object's own.
///
/// The object stays usable from Python, where anything can move its position: the caller's own
/// `seek` or `read`, or a second parser over the same object. So every read of the object seeks
/// first. A reader that trusts the stream to stay where its last read left it (as `MftParser`
/// does for sequential entries) would return the bytes of another entry.
///
/// Reads that follow one another are served from a window fetched in one go, as a `seek` and a
/// `read` call for every record cost more than parsing it. So bytes may be fetched up to one
/// window before they are asked for. A read from somewhere else (the record of a parent
/// directory, to resolve a path) fetches only what it asks for and leaves the window alone.
#[derive(Debug)]
pub struct PinnedFileLike {
    inner: PyFileLikeObject,
    position: u64,
    /// The bytes of the object from `window_start` on, as last fetched.
    window: Vec<u8>,
    window_start: u64,
    /// Where the last read ended, to tell the reads that follow one another.
    last_read_end: u64,
}

impl PinnedFileLike {
    fn new(inner: PyFileLikeObject) -> Self {
        PinnedFileLike {
            inner,
            position: 0,
            window: Vec::new(),
            window_start: 0,
            last_read_end: 0,
        }
    }

    /// The fetched bytes from the position on, which are none if it is outside the window.
    fn fetched(&self) -> &[u8] {
        self.position
            .checked_sub(self.window_start)
            .and_then(|offset| self.window.get(usize::try_from(offset).ok()?..))
            .unwrap_or_default()
    }

    /// Reads the object at the position, wherever anything else left it.
    fn fetch(&self, buf: &mut [u8]) -> io::Result<usize> {
        Python::attach(|py| {
            self.inner.py_seek(py, SeekFrom::Start(self.position))?;
            self.inner.py_read(py, buf)
        })
    }
}

impl Read for PinnedFileLike {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = if !self.fetched().is_empty() {
            let fetched = self.fetched();
            let read = fetched.len().min(buf.len());
            buf[..read].copy_from_slice(&fetched[..read]);
            read
        } else if self.position == self.last_read_end && buf.len() < WINDOW_SIZE {
            // Emptied first, so a failed fetch leaves no bytes under the wrong offset.
            self.window.clear();
            self.window_start = self.position;
            let mut window = vec![0; WINDOW_SIZE];
            let fetched = self.fetch(&mut window)?;
            window.truncate(fetched);
            self.window = window;

            let read = fetched.min(buf.len());
            buf[..read].copy_from_slice(&self.window[..read]);
            read
        } else {
            self.fetch(buf)?
        };
        self.position += read as u64;
        self.last_read_end = self.position;
        Ok(read)
    }
}

impl Seek for PinnedFileLike {
    /// Only moves the position kept here, which the next read is served from. Seeking from the
    /// end asks the object, as that is the only way to learn its size.
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
