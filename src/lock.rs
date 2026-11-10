use std::ops::{Deref, DerefMut};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread::{self, ThreadId};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::sync::MutexExt;

/// A mutex that is held while Python code runs (a file-like object's `read` and `seek`), and so
/// can be asked for again by the thread that holds it. That thread gets a `RuntimeError`, where
/// a plain mutex would have it wait for itself forever.
///
/// The GIL is released while waiting, so the thread holding the lock can run that Python code. A
/// poisoned lock is recovered, since a panic can only happen after an entry's bytes were read.
pub struct ReentryCheckedMutex<T> {
    value: Mutex<T>,
    /// The thread that holds `value`. Never held while waiting or running Python code.
    owner: Mutex<Option<ThreadId>>,
}

pub struct ReentryCheckedGuard<'a, T> {
    value: MutexGuard<'a, T>,
    owner: &'a Mutex<Option<ThreadId>>,
}

impl<T> ReentryCheckedMutex<T> {
    pub fn new(value: T) -> Self {
        ReentryCheckedMutex {
            value: Mutex::new(value),
            owner: Mutex::new(None),
        }
    }

    /// Locks, or raises `RuntimeError` if this thread already holds the lock. `what` names what
    /// is locked in the error.
    pub fn lock<'a>(&'a self, py: Python<'_>, what: &str) -> PyResult<ReentryCheckedGuard<'a, T>> {
        let this_thread = thread::current().id();
        // Only this thread sets the owner to itself, so the answer can't change under it.
        if *owner(&self.owner) == Some(this_thread) {
            return Err(PyRuntimeError::new_err(format!(
                "{what} is already in use by this thread: it was called again from Python code \
                 it was running, such as its file object's read() or seek()"
            )));
        }
        let value = self
            .value
            .lock_py_attached(py)
            .unwrap_or_else(PoisonError::into_inner);
        *owner(&self.owner) = Some(this_thread);
        Ok(ReentryCheckedGuard {
            value,
            owner: &self.owner,
        })
    }
}

fn owner(owner: &Mutex<Option<ThreadId>>) -> MutexGuard<'_, Option<ThreadId>> {
    owner.lock().unwrap_or_else(PoisonError::into_inner)
}

impl<T> Drop for ReentryCheckedGuard<'_, T> {
    fn drop(&mut self) {
        // Before `value` is unlocked, which happens when the fields are dropped.
        *owner(self.owner) = None;
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
