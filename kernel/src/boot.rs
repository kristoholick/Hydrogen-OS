//! Requests the kernel makes to the Limine bootloader. Limine finds them by scanning the
//! `.limine_requests` section (see linker-x86_64.ld) and fills in the responses before
//! jumping to `kmain`.

use limine::request::{
    BootloaderInfoRequest, ExecutableAddressRequest, FramebufferRequest, HhdmRequest, MemmapRequest,
    ModulesRequest, RsdpRequest,
};
use limine::{BaseRevision, RequestsEndMarker, RequestsStartMarker};

#[used]
#[unsafe(link_section = ".limine_requests_start")]
static REQUESTS_START: RequestsStartMarker = RequestsStartMarker::new();

/// Newest boot-protocol revision this crate understands.
#[used]
#[unsafe(link_section = ".limine_requests")]
pub static BASE_REVISION: BaseRevision = BaseRevision::new();

#[used]
#[unsafe(link_section = ".limine_requests")]
pub static BOOTLOADER_INFO: BootloaderInfoRequest = BootloaderInfoRequest::new();

#[used]
#[unsafe(link_section = ".limine_requests")]
pub static FRAMEBUFFER: FramebufferRequest = FramebufferRequest::new();

#[used]
#[unsafe(link_section = ".limine_requests")]
pub static MEMMAP: MemmapRequest = MemmapRequest::new();

#[used]
#[unsafe(link_section = ".limine_requests")]
pub static HHDM: HhdmRequest = HhdmRequest::new();

/// Where Limine put the kernel image (physical and virtual), needed to build page tables.
#[used]
#[unsafe(link_section = ".limine_requests")]
pub static EXECUTABLE_ADDRESS: ExecutableAddressRequest = ExecutableAddressRequest::new();

/// Where the firmware put the root of its ACPI tables.
#[used]
#[unsafe(link_section = ".limine_requests")]
static RSDP: RsdpRequest = RsdpRequest::new();

/// Physical address of the ACPI root pointer. (The protocol revision decides whether Limine
/// gives a physical or a direct-map address; both are accepted.)
pub fn rsdp_address() -> Option<u64> {
    let hhdm = HHDM.response()?.offset;
    let a = RSDP.response()?.address as u64;
    Some(if a >= hhdm { a - hhdm } else { a }).filter(|&a| a != 0)
}

pub fn memory_map() -> &'static [&'static limine::memmap::Entry] {
    MEMMAP.response().map_or(&[], |m| m.entries())
}

/// Files loaded next to the kernel: the user programs (until there is a file system).
#[used]
#[unsafe(link_section = ".limine_requests")]
static MODULES: ModulesRequest = ModulesRequest::new();

/// All boot modules: file name (without folder) and contents.
pub fn modules() -> impl Iterator<Item = (&'static str, &'static [u8])> {
    let files = MODULES.response().map_or(&[][..], |r| r.modules());
    files.iter().map(|f| (f.path().rsplit('/').next().unwrap_or(""), f.data()))
}

pub fn module(name: &str) -> Option<&'static [u8]> {
    modules().find(|(n, _)| *n == name).map(|(_, data)| data)
}

#[used]
#[unsafe(link_section = ".limine_requests_end")]
static REQUESTS_END: RequestsEndMarker = RequestsEndMarker::new();
