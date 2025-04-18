use crate::proc::ProcessContext;
use super::consts::*;
use crate::memory::gdt;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

pub unsafe fn register_idt(idt: &mut InterruptDescriptorTable) {
    idt[Interrupts::IrqBase as u8 + Irq::Timer as u8]
        .set_handler_fn(clock_handler)
        .set_stack_index(gdt::CLOCK); 
}

pub extern "C" fn clock(ctx: &mut ProcessContext) {
    crate::proc::switch(ctx);
    super::ack();
}

as_handler!(clock);