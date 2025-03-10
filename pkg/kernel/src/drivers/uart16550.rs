use core::fmt;
use core::arch::asm;
use x86::io::{inb, outb};

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
