//! Architecture layer. Only x86-64 for now; other CPUs get their own module later.
pub mod x86_64;

pub use x86_64::cpu::halt_forever;
