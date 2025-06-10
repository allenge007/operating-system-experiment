#![no_std]
#![no_main]

extern crate alloc;

mod services;
mod utils;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use lib::*;
use owo_colors::OwoColorize;

/// 定义简单的高亮函数，根据预定义命令高亮首个单词
fn highlight(input: &str) -> String {
    // 定义预期高亮的命令列表
    let commands = ["ps", "ls", "exec", "kill", "help", "clear", "exit", "cat", "lsapp"];
    // 尝试拆分输入，取第一个单词进行匹配
    if let Some((first, rest)) = input.split_once(' ') {
        for &cmd in commands.iter() {
            if first == cmd {
                return format!("{} {}", first.bright_green().bold(), rest);
            }
        }
        return format!("{} {}", first.red().bold(), rest);
    } else {
        // 如果输入不包含空格，则直接匹配整个字符串
        for &cmd in commands.iter() {
            if input == cmd {
                return input.bright_green().bold().to_string();
            }
        }
        return input.red().bold().to_string();
    }
}

fn read_line_history(history: &mut Vec<String>, history_index: &mut Option<usize>) -> String {
    let mut buffer = String::new();
    // cursor 表示当前光标在 buffer 内的位置（范围 0..=buffer.len()）
    let mut cursor: usize = 0;
    
    loop {
        if let Some(c) = stdin().read_key() {
            match c {
                // 回车：完成输入
                '\n' | '\r' => {
                    println!();
                    break;
                }
                // 退格：删除光标前的字符（如果存在），并更新光标位置
                '\x08' | '\x7F' => {
                    if cursor > 0 {
                        buffer.remove(cursor - 1);
                        cursor -= 1;
                    }
                    // 重绘当前输入行
                    print!("\r");
                    utils::print_prompt_sec();
                    print!("{}", highlight(&buffer));
                    // 清除从当前光标到行尾的残留字符
                    print!("\x1b[0K");
                    // 将光标移回到正确位置（即距离行尾移动光标：buffer.len()-cursor）
                    let shift = buffer.len().saturating_sub(cursor);
                    if shift > 0 {
                        print!("\x1b[{}D", shift);
                    }
                    stdout().flush();
                }
                // 处理 ESC 控制序列
                '\x1b' => {
                    if let Some('[') = stdin().read_key() {
                        if let Some(code) = stdin().read_key() {
                            match code {
                                'A' => {
                                    // 上箭头：显示上一条历史命令（如果存在）
                                    if !history.is_empty() {
                                        let idx = match history_index {
                                            Some(i) if *i > 0 => *i - 1,
                                            None => history.len() - 1,
                                            _ => 0,
                                        };
                                        *history_index = Some(idx);
                                        let cmd = history.get(idx).unwrap();
                                        buffer = cmd.clone();
                                        cursor = buffer.len();
                                        print!("\r");
                                        utils::print_prompt_sec();
                                        print!("{}", highlight(&buffer));
                                        print!("\x1b[0K");
                                        stdout().flush();
                                    }
                                }
                                'B' => {
                                    // 下箭头：显示下一条历史命令（如果存在）
                                    if !history.is_empty() {
                                        if let Some(i) = history_index {
                                            if *i < history.len() - 1 {
                                                *history_index = Some(*i + 1);
                                                let cmd = history.get(*history_index.as_ref().unwrap()).unwrap();
                                                buffer = cmd.clone();
                                                cursor = buffer.len();
                                                print!("\r");
                                                utils::print_prompt_sec();
                                                print!("{}", highlight(&buffer));
                                                print!("\x1b[0K");
                                                stdout().flush();
                                            } else {
                                                // 已处于最新状态，则清空输入
                                                *history_index = Some(history.len());
                                                buffer.clear();
                                                cursor = 0;
                                                print!("\r");
                                                utils::print_prompt_sec();
                                                print!("\x1b[0K");
                                                stdout().flush();
                                            }
                                        }
                                    }
                                }
                                'C' => {
                                    // 右箭头：如果光标未在行尾，则右移一格
                                    if cursor < buffer.len() {
                                        cursor += 1;
                                        // 直接移动光标右边一格
                                        print!("\x1b[1C");
                                        stdout().flush();
                                    }
                                }
                                'D' => {
                                    // 左箭头：如果光标未在行首，则左移一格
                                    if cursor > 0 {
                                        cursor -= 1;
                                        print!("\x1b[1D");
                                        stdout().flush();
                                    }
                                }
                                _ => {
                                    // 其它控制字符忽略
                                }
                            }
                        }
                    }
                }
                // 普通字符：过滤掉其他不可打印控制字符
                _ => {
                    if !c.is_control() || c == ' ' || c == '\t' {
                        // 插入字符到当前光标位置
                        buffer.insert(cursor, c);
                        cursor += 1;
                    }
                    // 重绘输入行：先回到行首，然后输出特殊提示符和经高亮处理后的缓冲区内容
                    print!("\r");
                    utils::print_prompt_sec();
                    print!("{}", highlight(&buffer));
                    print!("\x1b[0K");
                    // 将光标移动到正确位置（从行尾左移 buffer.len()-cursor 个字符）
                    let shift = buffer.len().saturating_sub(cursor);
                    if shift > 0 {
                        print!("\x1b[{}D", shift);
                    }
                    stdout().flush();
                }
            }
        }
    }
    buffer
}

fn main() -> isize {
    utils::show_welcome_text();
    let mut history: Vec<String> = Vec::new();
    // history_index 为当前选中的历史命令索引，初始时设为 history.len()
    let mut history_index: Option<usize> = Some(0);
    
    loop {
        utils::print_prompt();
        // 使用基于 read_key 的 read_line_history 实现实时回显与历史命令切换
        let input = read_line_history(&mut history, &mut history_index);
        let trimmed = input.trim();
        if !trimmed.is_empty() {
            history.push(trimmed.to_string());
            // 重置 history_index 到最新记录位置
            history_index = Some(history.len());
        }
        let line: Vec<&str> = trimmed.split(' ').collect();
        match line.get(0).unwrap_or(&"") {
            &"\x04" | &"exit" => {
                println!();
                break;
            }
            &"ps" => sys_stat(),
            &"ls" => {
                let path_to_list = if line.len() >= 2 {
                    line[1]
                } else {
                    "/" // Default to root directory if no path is specified
                };
                // Assuming lib::list_dir handles printing and syscall
                if let Err(e) = list_dir(path_to_list) {
                    errln!("ls: {}", e);
                }
            },
            &"cat" => {
                if line.len() < 2 {
                    println!("Usage: cat <file>");
                    continue;
                }
                services::cat_file(line[1]);
                println!();
            },
            &"lsapp" => sys_list_app(),
            &"exec" => {
                if line.len() < 2 {
                    println!("Usage: exec <file>");
                    continue;
                }
                services::exec(line[1]);
            }
            &"kill" => {
                if line.len() < 2 {
                    println!("Usage: kill <pid>");
                    continue;
                }
                let pid = line[1].to_string().parse::<u16>();
                if pid.is_err() {
                    errln!("Cannot parse pid");
                    continue;
                }
                services::kill(pid.unwrap());
            }
            &"help" => utils::show_help_text(),
            &"clear" => {
                print!("\x1b[1;1H\x1b[2J");
                stdout().flush();
            }
            other => {
                if other.is_empty() {
                    println!();
                    continue;
                }
                println!("Command not found: {}", other);
                println!("Type 'help' to see available commands.");
            }
        }
    }
    0
}

entry!(main);