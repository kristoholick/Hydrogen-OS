//! Interrupt Descriptor Table: what the CPU runs for each exception and interrupt.
//!
//! Every CPU exception has a handler. Fatal ones report and go to the crash screen;
//! the breakpoint exception is recoverable and returns. Vectors 32+ are hardware interrupts.

use super::gdt::{IST_DOUBLE_FAULT, IST_MACHINE_CHECK, IST_NMI};
use crate::{kprintln, mm};
use spin::Once;
use x86_64::registers::control::Cr2;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

pub const TIMER_VECTOR: u8 = 0x20;
pub const SPURIOUS_VECTOR: u8 = 0xFF;

static IDT: Once<InterruptDescriptorTable> = Once::new();

pub fn init() {
    let idt = IDT.call_once(|| {
        let mut idt = InterruptDescriptorTable::new();
        idt.divide_error.set_handler_fn(divide_error);
        idt.debug.set_handler_fn(debug);
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
            idt.non_maskable_interrupt.set_handler_fn(nmi).set_stack_index(IST_NMI);
            idt.machine_check.set_handler_fn(machine_check).set_stack_index(IST_MACHINE_CHECK);
        }
        idt[TIMER_VECTOR].set_handler_fn(timer_interrupt);
        idt[SPURIOUS_VECTOR].set_handler_fn(spurious_interrupt);
        idt
    });
    idt.load();
}

fn fatal(name: &str, f: &InterruptStackFrame, code: Option<u64>) -> ! {
    match code {
        Some(c) => panic!("EXCEPTION: {} (error code {:#x}) at rip={:#x} rsp={:#x}",
            name, c, f.instruction_pointer.as_u64(), f.stack_pointer.as_u64()),
        None => panic!("EXCEPTION: {} at rip={:#x} rsp={:#x}",
            name, f.instruction_pointer.as_u64(), f.stack_pointer.as_u64()),
    }
}

macro_rules! fatal_handler {
    ($fn:ident, $name:literal) => {
        extern "x86-interrupt" fn $fn(f: InterruptStackFrame) {
            fatal($name, &f, None)
        }
    };
    ($fn:ident, $name:literal, code) => {
        extern "x86-interrupt" fn $fn(f: InterruptStackFrame, code: u64) {
            fatal($name, &f, Some(code))
        }
    };
}

fatal_handler!(divide_error, "DIVIDE ERROR");
fatal_handler!(debug, "DEBUG");
fatal_handler!(overflow, "OVERFLOW");
fatal_handler!(bound_range_exceeded, "BOUND RANGE EXCEEDED");
fatal_handler!(invalid_opcode, "INVALID OPCODE");
fatal_handler!(device_not_available, "DEVICE NOT AVAILABLE");
fatal_handler!(nmi, "NON-MASKABLE INTERRUPT");
fatal_handler!(x87_floating_point, "X87 FLOATING POINT");
fatal_handler!(simd_floating_point, "SIMD FLOATING POINT");
fatal_handler!(virtualization, "VIRTUALIZATION");
fatal_handler!(hv_injection, "HYPERVISOR INJECTION");
fatal_handler!(invalid_tss, "INVALID TSS", code);
fatal_handler!(segment_not_present, "SEGMENT NOT PRESENT", code);
fatal_handler!(stack_segment_fault, "STACK SEGMENT FAULT", code);
fatal_handler!(general_protection_fault, "GENERAL PROTECTION FAULT", code);
fatal_handler!(alignment_check, "ALIGNMENT CHECK", code);
fatal_handler!(cp_protection, "CONTROL PROTECTION", code);
fatal_handler!(vmm_communication, "VMM COMMUNICATION", code);
fatal_handler!(security_exception, "SECURITY EXCEPTION", code);

extern "x86-interrupt" fn breakpoint(f: InterruptStackFrame) {
    kprintln!("[hydrogen] BREAKPOINT at rip={:#x} - handled, continuing", f.instruction_pointer.as_u64());
}

extern "x86-interrupt" fn page_fault(f: InterruptStackFrame, code: PageFaultErrorCode) {
    let addr = Cr2::read_raw();
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

extern "x86-interrupt" fn timer_interrupt(_f: InterruptStackFrame) {
    crate::time::on_tick();
    // Acknowledge first: the scheduler may switch to another thread, and without the EOI that
    // thread would never see another timer interrupt.
    super::apic::eoi();
    crate::sched::on_tick();
}

extern "x86-interrupt" fn spurious_interrupt(_f: InterruptStackFrame) {
    // Spurious APIC interrupts must not be acknowledged.
}
