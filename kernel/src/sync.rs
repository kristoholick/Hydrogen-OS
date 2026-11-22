//! Locks that are safe to use from both normal code and interrupt handlers.
//!
//! `IrqMutex` masks interrupts while held. Without that, a timer interrupt arriving while
//! code holds a lock — and whose handler wants the same lock — would deadlock forever.

use core::ops::{Deref, DerefMut};
use spin::{Mutex, MutexGuard};
use x86_64::instructions::interrupts;

pub struct IrqMutex<T> {
    inner: Mutex<T>,
}

pub struct IrqGuard<'a, T> {
    guard: Option<MutexGuard<'a, T>>,
    reenable: bool,
}

impl<T> IrqMutex<T> {
    pub const fn new(value: T) -> Self {
        Self { inner: Mutex::new(value) }
    }

    pub fn lock(&self) -> IrqGuard<'_, T> {
        let reenable = interrupts::are_enabled();
        interrupts::disable();
        IrqGuard { guard: Some(self.inner.lock()), reenable }
    }

    /// # Safety
    /// Only for the panic path: interrupts are off, one core runs, and the holder of the
    /// lock will never run again.
    pub unsafe fn force_unlock(&self) {
        // SAFETY: forwarded from the caller's guarantee above.
        unsafe { self.inner.force_unlock() }
    }
}

impl<T> Deref for IrqGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.guard.as_ref().unwrap()
    }
}

impl<T> DerefMut for IrqGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.guard.as_mut().unwrap()
    }
}

impl<T> Drop for IrqGuard<'_, T> {
    fn drop(&mut self) {
        drop(self.guard.take()); // release the lock first...
        if self.reenable {
            interrupts::enable(); // ...then let interrupts back in
        }
    }
}
