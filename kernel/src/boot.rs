//! Requests the kernel makes to the Limine bootloader. Limine finds them by scanning the
//! `.limine_requests` section (see linker-x86_64.ld) and fills in the responses before
//! jumping to `kmain`.

use limine::request::{BootloaderInfoRequest, FramebufferRequest, HhdmRequest, MemmapRequest};
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

#[used]
#[unsafe(link_section = ".limine_requests_end")]
static REQUESTS_END: RequestsEndMarker = RequestsEndMarker::new();
