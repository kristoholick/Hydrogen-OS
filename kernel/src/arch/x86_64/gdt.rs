//! Global Descriptor Table and Task State Segment.
//!
//! In 64-bit mode segments are mostly vestigial, but the CPU still needs: a code segment
//! for ring 0 (and later ring 3), and a TSS whose Interrupt Stack Table gives critical
//! exceptions their own emergency stacks — so a kernel stack overflow can still be reported.
//! The TSS also says which stack the CPU switches to when ring 3 is interrupted.

use core::cell::UnsafeCell;
use spin::Once;
use x86_64::VirtAddr;
use x86_64::instructions::segmentation::{CS, DS, ES, FS, GS, SS, Segment};
use x86_64::instructions::tables::load_tss;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;

/// Interrupt Stack Table slots (the CPU numbers them 1–7; the crate uses 0–6).
pub const IST_DOUBLE_FAULT: u16 = 0;
pub const IST_NMI: u16 = 1;
pub const IST_MACHINE_CHECK: u16 = 2;
/// Debug exceptions can, with some CPU trickery, arrive right at the `syscall` entry while the
/// stack pointer is still the user's; their own stack makes that harmless.
pub const IST_DEBUG: u16 = 3;
const IST_COUNT: usize = 4;
const IST_STACK_SIZE: usize = 16 * 1024;

#[repr(C, align(16))]
struct IstStacks(UnsafeCell<[[u8; IST_STACK_SIZE]; IST_COUNT]>);
// SAFETY: each stack is used only by the CPU while handling its one exception type;
// Rust code never reads or writes this memory directly.
unsafe impl Sync for IstStacks {}
static IST_STACKS: IstStacks = IstStacks(UnsafeCell::new([[0; IST_STACK_SIZE]; IST_COUNT]));

pub struct Selectors {
    pub kernel_code: SegmentSelector,
    pub kernel_data: SegmentSelector,
    pub user_data: SegmentSelector,
    pub user_code: SegmentSelector,
    pub tss: SegmentSelector,
}

struct Tss(UnsafeCell<TaskStateSegment>);
// SAFETY: written only by `init` and, with interrupts off on the one core, by
// `set_ring0_stack`; otherwise only the CPU reads it.
unsafe impl Sync for Tss {}
static TSS: Tss = Tss(UnsafeCell::new(TaskStateSegment::new()));
static GDT: Once<(GlobalDescriptorTable, Selectors)> = Once::new();

pub fn init() {
    let tss = TSS.0.get();
    let base = IST_STACKS.0.get() as u64;
    for i in 0..IST_COUNT {
        // stacks grow down: point at the top end of each one
        // SAFETY: called once at boot, before the TSS is loaded, so nothing else uses it yet.
        unsafe { (*tss).interrupt_stack_table[i] = VirtAddr::new(base + ((i + 1) * IST_STACK_SIZE) as u64) };
    }

    let (gdt, sel) = GDT.call_once(|| {
        let mut gdt = GlobalDescriptorTable::new();
        let kernel_code = gdt.append(Descriptor::kernel_code_segment());
        let kernel_data = gdt.append(Descriptor::kernel_data_segment());
        // user data before user code: the order `sysret` expects (Phase 3)
        let user_data = gdt.append(Descriptor::user_data_segment());
        let user_code = gdt.append(Descriptor::user_code_segment());
        // SAFETY: the TSS is a static, so the pointer stays valid forever.
        let tss = gdt.append(unsafe { Descriptor::tss_segment_unchecked(tss) });
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
        FS::set_reg(SegmentSelector::NULL);
        GS::set_reg(SegmentSelector::NULL);
        load_tss(sel.tss);
        // No local descriptor table: a program must not reach whatever the firmware left there.
        core::arch::asm!("lldt {0:x}", in(reg) 0u16, options(nostack, preserves_flags));
    }
}

pub fn selectors() -> &'static Selectors {
    &GDT.get().expect("GDT not initialised").1
}

/// Stack the CPU switches to when an interrupt or exception arrives in ring 3. Interrupts must
/// be off.
pub fn set_ring0_stack(top: u64) {
    // SAFETY: one core, interrupts off: the CPU reads this field only on an interrupt from
    // ring 3, which cannot happen while the kernel runs here.
    unsafe { (*TSS.0.get()).privilege_stack_table[0] = VirtAddr::new(top) };
}

/// Forget what a program put into its data segment registers, so the next one cannot see it.
/// (In 64-bit mode they have no other use; the kernel does not rely on them either.)
pub fn clear_data_segments() {
    // SAFETY: null selectors are valid in 64-bit mode, in ring 0 and ring 3.
    unsafe {
        DS::set_reg(SegmentSelector::NULL);
        ES::set_reg(SegmentSelector::NULL);
        FS::set_reg(SegmentSelector::NULL);
        GS::set_reg(SegmentSelector::NULL);
    }
}

/// Address range of the emergency stacks (for diagnostics).
pub fn ist_range() -> (u64, u64) {
    let base = IST_STACKS.0.get() as u64;
    (base, base + (IST_COUNT * IST_STACK_SIZE) as u64)
}
