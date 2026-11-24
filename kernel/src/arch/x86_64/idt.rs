//! Interrupt Descriptor Table: what the CPU runs for each exception and interrupt.
//!
//! Every CPU exception has a handler. In ring 3 it ends the program that caused it. In the kernel
//! fatal ones report and go to the crash screen;
//! the breakpoint exception is recoverable and returns. Vectors 32+ are hardware interrupts.

use super::cpu::{assert_flags_clean, close_smap};
use super::gdt::{IST_DEBUG, IST_DOUBLE_FAULT, IST_MACHINE_CHECK, IST_NMI};
use crate::{kprintln, mm};
use core::fmt;
use spin::Once;
use x86_64::PrivilegeLevel;
use x86_64::instructions::interrupts;
use x86_64::registers::control::Cr2;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

pub const TIMER_VECTOR: u8 = 0x20;
pub const SPURIOUS_VECTOR: u8 = 0xFF;

static IDT: Once<InterruptDescriptorTable> = Once::new();

pub fn init() {
    let idt = IDT.call_once(|| {
        let mut idt = InterruptDescriptorTable::new();
        idt.divide_error.set_handler_fn(divide_error);
        idt.breakpoint.set_handler_fn(breakpoint);
        idt.overflow.set_handler_fn(overflow);
        idt.bound_range_exceeded.set_handler_fn(bound_range_exceeded);
        idt.invalid_opcode.set_handler_fn(invalid_opcode);
        idt.device_not_available.set_handler_fn(device_not_available);
        idt.invalid_tss.set_handler_fn(invalid_tss);
        idt.segment_not_present.set_handler_fn(segment_not_present);
        idt.stack_segment_fault.set_handler_fn(stack_segment_fault);
        idt.general_protection_fault.set_handler_fn(general_protection_fault);
        idt.page_fault.set_handler_fn(page_fault);
        idt.x87_floating_point.set_handler_fn(x87_floating_point);
        idt.alignment_check.set_handler_fn(alignment_check);
        idt.simd_floating_point.set_handler_fn(simd_floating_point);
        idt.virtualization.set_handler_fn(virtualization);
        idt.cp_protection_exception.set_handler_fn(cp_protection);
        idt.hv_injection_exception.set_handler_fn(hv_injection);
        idt.vmm_communication_exception.set_handler_fn(vmm_communication);
        idt.security_exception.set_handler_fn(security_exception);
        // SAFETY: the IST indices exist in the TSS loaded by `gdt::init`, and each stack
        // is used by exactly one exception type.
        unsafe {
            idt.double_fault.set_handler_fn(double_fault).set_stack_index(IST_DOUBLE_FAULT);
            idt.debug.set_handler_fn(debug).set_stack_index(IST_DEBUG);
            idt.non_maskable_interrupt.set_handler_fn(nmi).set_stack_index(IST_NMI);
            idt.machine_check.set_handler_fn(machine_check).set_stack_index(IST_MACHINE_CHECK);
        }
        idt[TIMER_VECTOR].set_handler_fn(timer_interrupt);
        idt[SPURIOUS_VECTOR].set_handler_fn(spurious_interrupt);
        idt
    });
    idt.load();
}

/// Did the interrupted code run in ring 3?
fn from_user(f: &InterruptStackFrame) -> bool {
    f.code_segment.rpl() == PrivilegeLevel::Ring3
}

fn fatal(name: &str, f: &InterruptStackFrame, code: Option<u64>) -> ! {
    match code {
        Some(c) => panic!("EXCEPTION: {} (error code {:#x}) at rip={:#x} rsp={:#x}",
            name, c, f.instruction_pointer.as_u64(), f.stack_pointer.as_u64()),
        None => panic!("EXCEPTION: {} at rip={:#x} rsp={:#x}",
            name, f.instruction_pointer.as_u64(), f.stack_pointer.as_u64()),
    }
}

/// An exception in ring 3 ends that program; in the kernel it is fatal.
fn exception(name: &'static str, f: &InterruptStackFrame, code: Option<u64>) {
    if from_user(f) {
        interrupts::enable(); // on the program's own kernel stack, no lock held
        match code {
            Some(c) => user_fault(name, f, format_args!(" (error code {c:#x})")),
            None => user_fault(name, f, format_args!("")),
        }
    }
    fatal(name, f, code)
}

fn user_fault(name: &'static str, f: &InterruptStackFrame, details: fmt::Arguments) -> ! {
    close_smap();
    crate::proc::kill_current(name, format_args!("{} rip={:#x}", details, f.instruction_pointer.as_u64()))
}

macro_rules! handler {
    ($fn:ident, $name:literal) => {
        extern "x86-interrupt" fn $fn(f: InterruptStackFrame) {
            exception($name, &f, None)
        }
    };
    ($fn:ident, $name:literal, code) => {
        extern "x86-interrupt" fn $fn(f: InterruptStackFrame, code: u64) {
            exception($name, &f, Some(code))
        }
    };
}

handler!(divide_error, "DIVIDE ERROR");
handler!(overflow, "OVERFLOW");
handler!(bound_range_exceeded, "BOUND RANGE EXCEEDED");
handler!(invalid_opcode, "INVALID OPCODE");
handler!(device_not_available, "DEVICE NOT AVAILABLE");
handler!(x87_floating_point, "X87 FLOATING POINT");
handler!(simd_floating_point, "SIMD FLOATING POINT");
handler!(virtualization, "VIRTUALIZATION");
handler!(hv_injection, "HYPERVISOR INJECTION");
handler!(invalid_tss, "INVALID TSS", code);
handler!(segment_not_present, "SEGMENT NOT PRESENT", code);
handler!(stack_segment_fault, "STACK SEGMENT FAULT", code);
handler!(general_protection_fault, "GENERAL PROTECTION FAULT", code);
handler!(alignment_check, "ALIGNMENT CHECK", code);
handler!(cp_protection, "CONTROL PROTECTION", code);
handler!(vmm_communication, "VMM COMMUNICATION", code);
handler!(security_exception, "SECURITY EXCEPTION", code);

extern "x86-interrupt" fn nmi(f: InterruptStackFrame) {
    fatal("NON-MASKABLE INTERRUPT", &f, None) // a hardware problem, whoever was running
}

extern "x86-interrupt" fn debug(f: InterruptStackFrame) {
    if from_user(&f) {
        // Interrupts stay off: this runs on the shared emergency stack, so nobody else may run
        // before the program is gone.
        user_fault("DEBUG", &f, format_args!(""))
    }
    fatal("DEBUG", &f, None)
}

extern "x86-interrupt" fn breakpoint(f: InterruptStackFrame) {
    kprintln!("[hydrogen] BREAKPOINT at rip={:#x} - handled, continuing", f.instruction_pointer.as_u64());
}

extern "x86-interrupt" fn page_fault(f: InterruptStackFrame, code: PageFaultErrorCode) {
    let addr = Cr2::read_raw(); // before anything can cause another page fault
    if from_user(&f) {
        interrupts::enable();
        user_fault("PAGE FAULT", &f, format_args!(" at {:#x} ({:?})", addr, code));
    }
    let hint = if mm::vmm::is_stack_guard(addr) {
        " - kernel stack overflow (guard page)"
    } else if crate::sched::is_thread_stack_guard(addr) {
        " - thread stack overflow (guard page)"
    } else {
        ""
    };
    panic!("EXCEPTION: PAGE FAULT at {:#x} ({:?}) rip={:#x}{}",
        addr, code, f.instruction_pointer.as_u64(), hint);
}

extern "x86-interrupt" fn double_fault(f: InterruptStackFrame, _code: u64) -> ! {
    let rsp = f.stack_pointer.as_u64();
    let near = |guard: fn(u64) -> bool| guard(rsp) || guard(rsp.wrapping_sub(8));
    let hint = if near(mm::vmm::is_stack_guard) {
        " - kernel stack overflow caught on emergency stack"
    } else if near(crate::sched::is_thread_stack_guard) {
        " - thread stack overflow caught on emergency stack"
    } else {
        ""
    };
    panic!("EXCEPTION: DOUBLE FAULT at rip={:#x} rsp={:#x}{}", f.instruction_pointer.as_u64(), rsp, hint);
}

extern "x86-interrupt" fn machine_check(f: InterruptStackFrame) -> ! {
    panic!("EXCEPTION: MACHINE CHECK at rip={:#x} - hardware reported an error", f.instruction_pointer.as_u64());
}

extern "x86-interrupt" fn timer_interrupt(f: InterruptStackFrame) {
    close_smap();
    assert_flags_clean();
    crate::time::on_tick(from_user(&f));
    // Acknowledge first: the scheduler may switch to another thread, and without the EOI that
    // thread would never see another timer interrupt.
    super::apic::eoi();
    crate::sched::on_tick();
}

extern "x86-interrupt" fn spurious_interrupt(_f: InterruptStackFrame) {
    // Spurious APIC interrupts must not be acknowledged.
}
