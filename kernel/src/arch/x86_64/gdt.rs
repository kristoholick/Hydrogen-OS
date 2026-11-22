//! Global Descriptor Table and Task State Segment.
//!
//! In 64-bit mode segments are mostly vestigial, but the CPU still needs: a code segment
//! for ring 0 (and later ring 3), and a TSS whose Interrupt Stack Table gives critical
//! exceptions their own emergency stacks — so a kernel stack overflow can still be reported.

use core::cell::UnsafeCell;
use spin::Once;
use x86_64::VirtAddr;
use x86_64::instructions::segmentation::{CS, DS, ES, SS, Segment};
use x86_64::instructions::tables::load_tss;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;

/// Interrupt Stack Table slots (the CPU numbers them 1–7; the crate uses 0–6).
pub const IST_DOUBLE_FAULT: u16 = 0;
pub const IST_NMI: u16 = 1;
pub const IST_MACHINE_CHECK: u16 = 2;
const IST_COUNT: usize = 3;
const IST_STACK_SIZE: usize = 16 * 1024;

#[repr(C, align(16))]
struct IstStacks(UnsafeCell<[[u8; IST_STACK_SIZE]; IST_COUNT]>);
// SAFETY: each stack is used only by the CPU while handling its one exception type;
// Rust code never reads or writes this memory directly.
unsafe impl Sync for IstStacks {}
static IST_STACKS: IstStacks = IstStacks(UnsafeCell::new([[0; IST_STACK_SIZE]; IST_COUNT]));

#[allow(dead_code)] // user segments are used from Phase 3 (ring-3 processes)
pub struct Selectors {
    pub kernel_code: SegmentSelector,
    pub kernel_data: SegmentSelector,
    pub user_data: SegmentSelector,
    pub user_code: SegmentSelector,
    pub tss: SegmentSelector,
}

static TSS: Once<TaskStateSegment> = Once::new();
static GDT: Once<(GlobalDescriptorTable, Selectors)> = Once::new();

pub fn init() {
    let tss = TSS.call_once(|| {
        let mut tss = TaskStateSegment::new();
        let base = IST_STACKS.0.get() as u64;
        for i in 0..IST_COUNT {
            // stacks grow down: point at the top end of each one
            tss.interrupt_stack_table[i] = VirtAddr::new(base + ((i + 1) * IST_STACK_SIZE) as u64);
        }
        tss
    });

    let (gdt, sel) = GDT.call_once(|| {
        let mut gdt = GlobalDescriptorTable::new();
        let kernel_code = gdt.append(Descriptor::kernel_code_segment());
        let kernel_data = gdt.append(Descriptor::kernel_data_segment());
        // user data before user code: the order `sysret` expects (Phase 3)
        let user_data = gdt.append(Descriptor::user_data_segment());
        let user_code = gdt.append(Descriptor::user_code_segment());
        let tss = gdt.append(Descriptor::tss_segment(tss));
        (gdt, Selectors { kernel_code, kernel_data, user_data, user_code, tss })
    });

    gdt.load();
    // SAFETY: the selectors index valid descriptors in the GDT just loaded, which is 'static.
    // Reloading CS/SS/DS/ES points the CPU at our segments instead of Limine's; loading the
    // TSS activates the emergency stacks.
    unsafe {
        CS::set_reg(sel.kernel_code);
        SS::set_reg(sel.kernel_data);
        DS::set_reg(sel.kernel_data);
        ES::set_reg(sel.kernel_data);
        load_tss(sel.tss);
    }
}

/// Address range of the emergency stacks (for diagnostics).
pub fn ist_range() -> (u64, u64) {
    let base = IST_STACKS.0.get() as u64;
    (base, base + (IST_COUNT * IST_STACK_SIZE) as u64)
}
