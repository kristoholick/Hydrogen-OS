//! Device drivers. Each one stays small and owns its hardware exclusively.
pub mod acpi;
pub mod framebuffer;
pub mod pci;
pub mod serial;
