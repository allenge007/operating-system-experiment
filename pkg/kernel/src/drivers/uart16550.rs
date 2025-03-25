use core::fmt;
use core::arch::asm;
use x86::io::{inb, outb};
use bitflags::bitflags;

bitflags! {
    /// UART 16550 的 LCR (Line Control Register) 寄存器控制标志
    pub struct LcrFlags: u8 {
        /// DLAB: Divisor Latch Access Bit, 用于访问波特率分频寄存器
        const DLAB           = 0x80;
        /// 设置字长 5 位 (0b00)
        const WORD_LENGTH_5  = 0x00;
        /// 设置字长 6 位 (0b01)
        const WORD_LENGTH_6  = 0x01;
        /// 设置字长 7 位 (0b10)
        const WORD_LENGTH_7  = 0x02;
        /// 设置字长 8 位 (0b11)
        const WORD_LENGTH_8  = 0x03;
        /// STOP: 设置停止位，若设置则使用 2 个停止位（5位字长时为 1.5 个停止位）
        const STOP_BITS      = 0x04;
        /// PARITY_ENABLE: 启用奇偶校验
        const PARITY_ENABLE  = 0x08;
        /// EVEN_PARITY: 偶校验（与 PARITY_ENABLE 配合使用）
        const EVEN_PARITY    = 0x10;
        /// STICK_PARITY: 固定奇偶模式（与 PARITY_ENABLE 配合使用）
        const STICK_PARITY   = 0x20;
    }
}
/// A port-mapped UART 16550 serial interface.
pub struct SerialPort {
    port: u16,
}

impl SerialPort {
    pub const fn new(port: u16) -> Self {
        Self { port }
    }

    /// Initializes the serial port.
    pub fn init(&self) {
        // FIXME: Initialize the serial port
        unsafe {
            // Disable interrupts
            outb(self.port + 1, 0x00);
            // Enable DLAB (set baud rate divisor)
            outb(self.port + 3, 0x80);
            // Set divisor to 1 (lo byte) 115200 baud
            outb(self.port + 0, 0x01);
            // Set divisor hi byte to 0
            outb(self.port + 1, 0x00);
            // 8 bits, no parity, one stop bit
            outb(self.port + 3, 0x03);
            // Enable FIFO, clear them, with 14-byte threshold
            outb(self.port + 2, 0xC7);
            // IRQs enabled, RTS/DSR set
            outb(self.port + 4, 0x0B);
            // Enable interrupts
            outb(self.port + 1, 0x01);
        }
    }

    /// Sends a byte on the serial port.
    pub fn send(&mut self, data: u8) {
        // FIXME: Send a byte on the serial port
        while unsafe { inb(self.port + 5) } & 0x20 == 0 {}
        unsafe {
            outb(self.port, data);
        }
    }

    /// Receives a byte on the serial port no wait.
    pub fn receive(&mut self) -> Option<u8> {
        // FIXME: Receive a byte on the serial port no wait
        if unsafe { inb(self.port + 5) } & 0x01 != 0 {
            return Some(unsafe { inb(self.port) });
        }
        None
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.send(byte);
        }
        Ok(())
    }
}
