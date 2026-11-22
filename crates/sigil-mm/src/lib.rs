//! Memory-management algorithms used by the SIGIL kernel.
//!
//! Nothing here touches hardware: the kernel supplies the memory, these types only keep
//! the books. That is what lets `cargo test` check them on the host in milliseconds.

#![cfg_attr(not(test), no_std)]

pub mod bitmap;
pub mod heap;

/// Size of one physical frame / virtual page.
pub const PAGE_SIZE: usize = 4096;
