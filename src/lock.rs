use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::sync::MutexExt;

/// No thread: tokens start at 1.
const NO_OWNER: u64 = 0;

/// A number for the current thread, different from every other thread's.
fn thread_token() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(NO_OWNER + 1);
    thread_local! {
        static TOKEN: u64 = NEXT.fetch_add(1, Ordering::Relaxed);
    }
    TOKEN.with(|token| *token)
}

/// A mutex that is held while Python code runs (a file-like object's `read` and `seek`), and so
/// can be asked for again by the thread that holds it. That thread gets a `RuntimeError`, where
/// a plain mutex would have it wait for itself forever.
///
/// The GIL is released while waiting, so the thread holding the lock can run that Python code. A
/// poisoned lock is recovered, since a panic can only happen after an entry's bytes were read.
pub struct ReentryCheckedMutex<T> {
    value: Mutex<T>,
    /// The token of the thread that holds `value`.
    owner: AtomicU64,
}

pub struct ReentryCheckedGuard<'a, T> {
    value: MutexGuard<'a, T>,
    owner: &'a AtomicU64,
}

impl<T> ReentryCheckedMutex<T> {
    pub fn new(value: T) -> Self {
        ReentryCheckedMutex {
            value: Mutex::new(value),
            owner: AtomicU64::new(NO_OWNER),
        }
    }

    /// Locks, or raises `RuntimeError` if this thread already holds the lock. `what` names what
    /// is locked in the error.
    pub fn lock<'a>(&'a self, py: Python<'_>, what: &str) -> PyResult<ReentryCheckedGuard<'a, T>> {
        let this_thread = thread_token();
        // Only this thread sets the owner to its token, so the answer can't change under it.
        if self.owner.load(Ordering::Relaxed) == this_thread {
            return Err(PyRuntimeError::new_err(format!(
                "{what} is already in use by this thread: it was called again from Python code \
                 it was running, such as its file object's read() or seek()"
            )));
        }
        let value = self
            .value
            .lock_py_attached(py)
            .unwrap_or_else(PoisonError::into_inner);
        self.owner.store(this_thread, Ordering::Relaxed);
        Ok(ReentryCheckedGuard {
            value,
            owner: &self.owner,
        })
    }
}

impl<T> Drop for ReentryCheckedGuard<'_, T> {
    fn drop(&mut self) {
        // Before `value` is unlocked, which happens when the fields are dropped.
        self.owner.store(NO_OWNER, Ordering::Relaxed);
    }
}

impl<T> Deref for ReentryCheckedGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> DerefMut for ReentryCheckedGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}
