#![no_std]
extern crate alloc;

use core::hint::spin_loop;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;
use core::str;
use alloc::borrow::ToOwned;
use crossbeam_queue::ArrayQueue;
use lazy_static::lazy_static;

// 假设内核环境中提供了类似的输出宏，输出到串口或其他终端设备
// 例如：print! 和 warn! 宏
// 如果没有，请替换为你自己的输出机制

/// 定义输入数据类型，此处使用 u8 表示单个字符的 ASCII 码
pub type Key = u8;

lazy_static! {
    // 初始化一个大小为 128 的无锁队列作为输入缓冲区
    static ref INPUT_BUF: ArrayQueue<Key> = ArrayQueue::new(128);
}

/// 将一个键值压入输入缓冲区
#[inline]
pub fn push_key(key: Key) {
    if INPUT_BUF.push(key).is_err() {
        // 如果缓冲区已满，输出警告（请确保 warn! 宏已在内核中定义）
        warn!("Input buffer is full. Dropping key '{:?}'", key);
    }
}

/// 尝试从输入缓冲区非阻塞地取出一个键值
#[inline]
pub fn try_pop_key() -> Option<Key> {
    INPUT_BUF.pop()
}

/// 阻塞地从输入缓冲区获取一个键值，直到有数据为止
pub fn pop_key() -> Key {
    loop {
        if let Some(key) = try_pop_key() {
            return key;
        }
        spin_loop();
    }
}

pub fn get_line() -> String {
    // 存放最终输入的所有字节
    let mut input_buffer: Vec<u8> = Vec::with_capacity(128);
    let prompt = "> ";

    // 初次打印提示符
    print!("{}", prompt);

    loop {
        let key = pop_key();
        if key == b'\n' || key == b'\r' {
            print!("\n");
            break;
        } else if key == 0x08 || key == 0x7F {
            // 处理退格／删除键：利用 UTF-8 解码安全删除最后一个字符
            if !input_buffer.is_empty() {
                if let Ok(s) = core::str::from_utf8(&input_buffer) {
                    if let Some((idx, _)) = s.char_indices().rev().next() {
                        input_buffer.truncate(idx);
                    } else {
                        input_buffer.pop();
                    }
                } else {
                    input_buffer.pop();
                }
            }
        } else {
            input_buffer.push(key);
        }

        // 尝试将 input_buffer 解码为 UTF-8 字符串
        let current_input: &str = match core::str::from_utf8(&input_buffer) {
            Ok(s) => s,
            Err(_) => "<invalid utf8>",
        };

        // 使用 ANSI 控制序列 "\x1B[K" 清除行尾内容，
        // 再用 "\r" 将光标移到行首，刷新整个行显示：提示符 + 当前输入
        print!("\r\x1B[K{}{}", prompt, current_input);
    }

    match core::str::from_utf8(&input_buffer) {
        Ok(s) => s.to_owned(),
        Err(_) => String::from("<invalid utf8>"),
    }
}