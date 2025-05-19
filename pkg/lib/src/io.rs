use crate::*;
use alloc::string::{String, ToString};
use alloc::vec;
use core::fmt::{self, Write};
use spin::{Mutex, Lazy};

pub struct Stdin;
pub struct Stdout {
    buffer: String,
}
pub struct Stderr;

impl Stdin {
    fn new() -> Self {
        Self
    }

    fn is_utf8(ch: u8) -> bool {
        ch & 0x80 == 0 || ch & 0xE0 == 0xC0 || ch & 0xF0 == 0xE0 || ch & 0xF8 == 0xF0
    }

    fn to_utf8(&self, ch: u8) -> u32 {
        let mut codepoint = 0;

        if ch & 0x80 == 0 {
            codepoint = ch as u32;
        } else if ch & 0xE0 == 0xC0 {
            codepoint = ((ch & 0x1F) as u32) << 6;
            codepoint |= (self.pop_key() & 0x3F) as u32;
        } else if ch & 0xF0 == 0xE0 {
            codepoint = ((ch & 0x0F) as u32) << 12;
            codepoint |= ((self.pop_key() & 0x3F) as u32) << 6;
            codepoint |= (self.pop_key() & 0x3F) as u32;
        } else if ch & 0xF8 == 0xF0 {
            codepoint = ((ch & 0x07) as u32) << 18;
            codepoint |= ((self.pop_key() & 0x3F) as u32) << 12;
            codepoint |= ((self.pop_key() & 0x3F) as u32) << 6;
            codepoint |= (self.pop_key() & 0x3F) as u32;
        }

        codepoint
    }

    fn try_read_key_with_buf(&self, buf: &mut [u8]) -> Option<u8> {
        if let Some(bytes) = sys_read(0, buf) {
            if bytes == 1 {
                return Some(buf[0]);
            }
        }
        None
    }

    fn pop_key(&self) -> u8 {
        let mut buf = [0];
        loop {
            if let Some(key) = self.try_read_key_with_buf(&mut buf) {
                return key;
            }
        }
    }

    pub fn read_line(&self) -> String {
        // FIXME: allocate string
        // FIXME: read from input buffer
        //       - maybe char by char?
        // FIXME: handle backspace / enter...
        // FIXME: return string
        let mut string = String::new();
        loop {
            let ch = self.pop_key();

            match ch {
                0x0d => {
                    let _ = stdout().write_str("\n");
                    break;
                }
                0x03 => {
                    string.clear();
                    break;
                }
                0x08 | 0x7F => {
                    if !string.is_empty() {
                        let _ = stdout().write_str("\x08 \x08");
                        string.pop();
                    }
                }
                _ => {
                    if Self::is_utf8(ch) {
                        let utf_char = char::from_u32(self.to_utf8(ch)).unwrap();
                        string.push(utf_char);
                        print! {"{}", utf_char};
                    } else {
                        string.push(ch as char);
                        print!("{}", ch as char);
                    }
                }
            }
        }
        println!();
        string
    }

    pub fn read_key(&self) -> Option<char> {
        let ch = self.pop_key();
        if Self::is_utf8(ch) {
            let cp = self.to_utf8(ch);
            char::from_u32(cp)
        } else {
            Some(ch as char)
        }
    }
}

impl Stdout {
    fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    pub fn flush(&mut self) {
        if !self.buffer.is_empty() {
            sys_write(1, self.buffer.as_bytes());
            self.buffer.clear();
        }
    }
}

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.buffer.push_str(s);
        if self.buffer.len() > 1024 {
            self.flush();
        }
        Ok(())
    }
}

// 使用 spin::Lazy 延迟初始化全局 Stdout 实例，避免直接在静态变量中调用非 const 函数
static GLOBAL_STDOUT: Lazy<Mutex<Stdout>> = Lazy::new(|| Mutex::new(Stdout::new()));

pub fn stdout() -> spin::MutexGuard<'static, Stdout> {
    GLOBAL_STDOUT.lock()
}

impl Stderr {
    fn new() -> Self {
        Self
    }

    pub fn write(&self, s: &str) {
        sys_write(2, s.as_bytes());
    }
}

pub fn stdin() -> Stdin {
    Stdin::new()
}

pub fn stderr() -> Stderr {
    Stderr::new()
}
