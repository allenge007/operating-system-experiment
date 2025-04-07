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
use spin::Mutex;
use unicode_width::UnicodeWidthStr;

// 假设内核环境中提供了类似的输出宏，输出到串口或其他终端设备
// 例如：print! 和 warn! 宏
// 如果没有，请替换为你自己的输出机制

/// 定义输入数据类型，此处使用 u8 表示单个字符的 ASCII 码
pub type Key = u8;

lazy_static! {
    // 初始化一个大小为 128 的无锁队列作为输入缓冲区
    static ref INPUT_BUF: ArrayQueue<Key> = ArrayQueue::new(128);

    pub static ref HISTORY: Mutex<Vec<String>> = Mutex::new(Vec::new());
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
    let mut input_buffer: Vec<u8> = Vec::with_capacity(128);
    let prompt = "> ";
    let mut history_index: Option<usize> = None;
    // 记录光标在 input_buffer 中的字节索引（必须始终在有效字符边界上）
    let mut cursor_pos: usize = 0;

    print!("{}", prompt);

    loop {
        let key = pop_key();

        // 检查是否为转义序列（ESC 开头）
        if key == 0x1B {
            let second = pop_key();
            let third = pop_key();
            if second == b'[' {
                match third {
                    b'A' => {
                        // 上箭头：历史向上
                        let history = HISTORY.lock();
                        if !history.is_empty() {
                            history_index = match history_index {
                                None => Some(history.len() - 1),
                                Some(idx) if idx > 0 => Some(idx - 1),
                                Some(idx) => Some(idx),
                            };
                            if let Some(idx) = history_index {
                                input_buffer.clear();
                                input_buffer.extend_from_slice(history[idx].as_bytes());
                                // 历史加载后，将光标置于末尾
                                cursor_pos = input_buffer.len();
                            }
                        }
                    }
                    b'B' => {
                        // 下箭头：历史向下
                        let history = HISTORY.lock();
                        if !history.is_empty() {
                            history_index = match history_index {
                                Some(idx) if idx < history.len() - 1 => Some(idx + 1),
                                _ => None,
                            };
                            input_buffer.clear();
                            if let Some(idx) = history_index {
                                input_buffer.extend_from_slice(history[idx].as_bytes());
                            }
                            cursor_pos = input_buffer.len();
                        }
                    }
                    b'D' => {
                        // 左箭头：向左移动光标
                        if cursor_pos > 0 {
                            if let Ok(s) = core::str::from_utf8(&input_buffer) {
                                if let Some((idx, _)) = s[..cursor_pos].char_indices().rev().next() {
                                    cursor_pos = idx;
                                } else {
                                    cursor_pos -= 1;
                                }
                            } else {
                                cursor_pos -= 1;
                            }
                        }
                    }
                    b'C' => {
                        // 右箭头：向右移动光标
                        if cursor_pos < input_buffer.len() {
                            if let Ok(s) = core::str::from_utf8(&input_buffer) {
                                if let Some((rel_idx, ch)) = s[cursor_pos..].char_indices().next() {
                                    cursor_pos += rel_idx + ch.len_utf8();
                                } else {
                                    cursor_pos = input_buffer.len();
                                }
                            } else {
                                cursor_pos += 1;
                            }
                        }
                    }
                    _ => {
                        // 忽略其他转义序列
                        continue;
                    }
                }
                // 每个转义序列处理后，刷新显示并定位光标
                let current_input = core::str::from_utf8(&input_buffer).unwrap_or("<invalid utf8>");
                print!("\r\x1B[K{}{}", prompt, current_input);
                // 安全地获取前 cursor_pos 子串，避免越界
                let safe_cursor_pos = if cursor_pos > current_input.len() { current_input.len() } else { cursor_pos };
                let displayed_cursor = current_input.get(..safe_cursor_pos).unwrap_or("").width();
                print!("\r\x1B[{}C", prompt.width() + displayed_cursor);
                continue;
            }
        }

        if key == b'\n' || key == b'\r' {
            // 回车，输入结束。刷新行后退出循环
            print!("\n");
            break;
        } else if key == 0x08 || key == 0x7F {
            // 退格：删除光标前一个字符（按 UTF-8 分界安全删除）
            if cursor_pos > 0 {
                if let Ok(s) = core::str::from_utf8(&input_buffer) {
                    if let Some((idx, _)) = s[..cursor_pos].char_indices().rev().next() {
                        input_buffer.drain(idx..cursor_pos);
                        cursor_pos = idx;
                    } else {
                        input_buffer.drain((cursor_pos - 1)..cursor_pos);
                        cursor_pos -= 1;
                    }
                } else {
                    input_buffer.drain((cursor_pos - 1)..cursor_pos);
                    cursor_pos -= 1;
                }
            }
        } else {
            // 普通字符输入：插入到 cursor_pos 处，并移动光标
            history_index = None;
            input_buffer.insert(cursor_pos, key);
            cursor_pos += 1;
        }

        let current_input = core::str::from_utf8(&input_buffer).unwrap_or("<invalid utf8>");
        // 刷新整行显示：\r 回到行首，\x1B[K 清除行尾，然后打印提示符+内容
        print!("\r\x1B[K{}{}", prompt, current_input);
        // 同样安全地计算光标位置
        let safe_cursor_pos = if cursor_pos > current_input.len() { current_input.len() } else { cursor_pos };
        let displayed_cursor = current_input.get(..safe_cursor_pos).unwrap_or("").width();
        print!("\r\x1B[{}C", prompt.width() + displayed_cursor);
    }

    let final_command = core::str::from_utf8(&input_buffer)
        .unwrap_or("<invalid utf8>")
        .to_owned();
    if !final_command.is_empty() {
        HISTORY.lock().push(final_command.clone());
    }

    final_command
}