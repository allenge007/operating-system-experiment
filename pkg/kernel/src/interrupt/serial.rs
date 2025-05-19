use super::consts::*;
use crate::drivers::input::push_key;
use x86_64::structures::idt::InterruptDescriptorTable;
use x86_64::structures::idt::InterruptStackFrame;

/// 使用 UART 16550 的基地址（假设为 COM1，即 0x3F8）
const UART_PORT: u16 = 0x3F8;

/// 注册串口中断处理函数
pub unsafe fn register_idt(idt: &mut InterruptDescriptorTable) {
    idt[Interrupts::IrqBase as u8 + Irq::Serial0 as u8]
        .set_handler_fn(serial_handler);
}

/// 串口中断处理函数
pub extern "x86-interrupt" fn serial_handler(_st: InterruptStackFrame) {
    receive();
    super::ack(super::consts::Irq::Serial0 as u8);
}

/// 从串口 16550 读取数据并放入输入缓冲区
fn receive() {
    unsafe {
        // 检查 Line Status Register (LSR)（端口 UART_PORT + 5）中 Data Ready 位是否为 1
        if inb(UART_PORT + 5) & 1 != 0 {
            // 读取接收数据寄存器（UART_PORT 处）
            let ch = inb(UART_PORT);
            push_key(ch);
        }
    }
}

/// 从指定端口读取一个字节
#[inline]
unsafe fn inb(port: u16) -> u8 {
    let mut value: u8;
    unsafe {
        core::arch::asm!(
            "in al, dx",
            out("al") value,
            in("dx") port,
            options(nomem, nostack, preserves_flags)
        );       
    }
    value
}