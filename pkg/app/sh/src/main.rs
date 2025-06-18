#![no_std]
#![no_main]

extern crate alloc;

mod services;
mod utils; // 确保 utils 模块被正确导入

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use lib::*;
use owo_colors::OwoColorize;
use alloc::borrow::Cow;

/// 定义简单的高亮函数，根据预定义命令高亮首个单词
fn highlight(input: &str) -> String {
    // 定义预期高亮的命令列表
    let commands = ["ps", "ls", "exec", "kill", "help", "clear", "exit", "cat", "lsapp", "cd", "pwd"]; // 添加 cd 和 pwd
    // 尝试拆分输入，取第一个单词进行匹配
    if let Some((first, rest)) = input.split_once(' ') {
        for &cmd in commands.iter() {
            if first == cmd {
                return format!("{} {}", first.bright_green().bold(), rest);
            }
        }
        // 如果命令不在列表中，但有参数，则命令部分标红
        return format!("{} {}", first.red().bold(), rest);
    } else {
        // 如果输入不包含空格，则直接匹配整个字符串
        for &cmd in commands.iter() {
            if input == cmd {
                return input.bright_green().bold().to_string();
            }
        }
        // 如果整个输入都不是已知命令，则整个输入标红
        return input.red().bold().to_string();
    }
}

fn read_line_history(
    history: &mut Vec<String>,
    history_index: &mut Option<usize>,
    current_cwd_for_prompt: &str // 添加参数以传递给 utils::print_prompt_sec
) -> String {
    let mut buffer = String::new();
    // cursor 表示当前光标在 buffer 内的位置（范围 0..=buffer.len()）
    let mut cursor: usize = 0;
    
    loop {
        if let Some(c) = stdin().read_key() {
            match c {
                // 回车：完成输入
                '\n' | '\r' => {
                    println!(); // 确保命令执行后输出在新的一行
                    break;
                }
                // 退格：删除光标前的字符（如果存在），并更新光标位置
                '\x08' | '\x7F' => { // Backspace or Delete
                    if cursor > 0 {
                        buffer.remove(cursor - 1);
                        cursor -= 1;
                    
                        // 重绘当前输入行
                        print!("\r"); // 回到行首
                        utils::print_prompt_sec(); // 使用带 CWD 的 sec prompt
                        print!("{}", highlight(&buffer));
                        print!("\x1b[0K"); // 清除从当前光标到行尾的残留字符
                        // 将光标移回到正确位置
                        let shift = buffer.len().saturating_sub(cursor);
                        if shift > 0 {
                            print!("\x1b[{}D", shift);
                        }
                        stdout().flush();
                    }
                }
                // 处理 ESC 控制序列 (箭头键)
                '\x1b' => {
                    if let Some('[') = stdin().read_key() {
                        if let Some(code) = stdin().read_key() {
                            match code {
                                'A' => { // 上箭头
                                    if !history.is_empty() {
                                        let idx = match history_index {
                                            Some(i) if *i > 0 => *i - 1,
                                            None => history.len() - 1, // 如果是 None，从最后一条开始
                                            _ => 0, // 如果是 Some(0)，保持在 0
                                        };
                                        *history_index = Some(idx);
                                        if let Some(cmd) = history.get(idx) {
                                            buffer = cmd.clone();
                                            cursor = buffer.len();
                                            print!("\r");
                                            utils::print_prompt_sec();
                                            print!("{}", highlight(&buffer));
                                            print!("\x1b[0K");
                                            stdout().flush();
                                        }
                                    }
                                }
                                'B' => { // 下箭头
                                    if !history.is_empty() {
                                        if let Some(i) = *history_index {
                                            if i < history.len() -1 {
                                                let next_idx = i + 1;
                                                *history_index = Some(next_idx);
                                                if let Some(cmd) = history.get(next_idx) {
                                                    buffer = cmd.clone();
                                                    cursor = buffer.len();
                                                }
                                            } else { // 已经是最后一条或更新的命令，清空 buffer
                                                *history_index = Some(history.len()); // 指向新命令的位置
                                                buffer.clear();
                                                cursor = 0;
                                            }
                                            print!("\r");
                                            utils::print_prompt_sec();
                                            print!("{}", highlight(&buffer));
                                            print!("\x1b[0K");
                                            stdout().flush();
                                        }
                                    }
                                }
                                'C' => { // 右箭头
                                    if cursor < buffer.len() {
                                        cursor += 1;
                                        print!("\x1b[1C"); // 光标右移
                                        stdout().flush();
                                    }
                                }
                                'D' => { // 左箭头
                                    if cursor > 0 {
                                        cursor -= 1;
                                        print!("\x1b[1D"); // 光标左移
                                        stdout().flush();
                                    }
                                }
                                _ => { /* 其他 ESC 序列忽略 */ }
                            }
                        }
                    }
                }
                // 普通字符
                _ => {
                    // 过滤掉其他不可打印控制字符, 但允许空格和制表符
                    if !c.is_control() || c == ' ' || c == '\t' {
                        buffer.insert(cursor, c);
                        cursor += 1;
                        
                        // 重绘输入行
                        print!("\r");
                        utils::print_prompt_sec();
                        print!("{}", highlight(&buffer));
                        print!("\x1b[0K"); // 清除光标到行尾
                        // 将光标移动到正确位置
                        let shift = buffer.len().saturating_sub(cursor);
                        if shift > 0 {
                            print!("\x1b[{}D", shift);
                        }
                        stdout().flush();
                    }
                }
            }
        }
    }
    buffer
}

// normalize_path 函数 (保持你之前的版本或我建议的改进版本)
fn normalize_path(current_dir: &str, target_path: &str) -> String {
    let mut components: Vec<Cow<str>> = Vec::new();
    let mut is_absolute = false;

    if target_path.starts_with('/') {
        is_absolute = true;
    } else {
        is_absolute = current_dir.starts_with('/'); // CWD 应该是绝对的
        for part in current_dir.trim_matches('/').split('/').filter(|s| !s.is_empty()) {
            components.push(part.into());
        }
    }

    for part_str in target_path.split('/').filter(|s| !s.is_empty()) {
        match part_str {
            "." => { /* no-op */ }
            ".." => {
                if !components.is_empty() {
                    components.pop();
                } else if is_absolute {
                    // 绝对路径下，components 为空表示在根目录，".." 不改变它
                }
                // 如果不是绝对路径且 components 为空，".." 的行为取决于具体策略
                // 这里我们假设它不应该发生，因为 CWD 总是绝对的
            }
            _ => {
                components.push(part_str.into());
            }
        }
    }

    if is_absolute {
        if components.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", components.join("/"))
        }
    } else {
        // 理论上，如果 CWD 总是绝对的，这里不应该被执行
        if components.is_empty() {
            ".".to_string() 
        } else {
            components.join("/")
        }
    }
}


fn main() -> isize {
    utils::show_welcome_text();
    let mut history: Vec<String> = Vec::new();
    let mut history_index: Option<usize> = Some(history.len()); // 初始化为指向新命令的位置
    let mut current_working_directory = String::from("/");

    loop {
        // 调用 utils::print_prompt 并传递 CWD
        utils::print_prompt(&current_working_directory); // <--- 使用 utils::print_prompt
        lib::stdout().flush();

        // 调用 read_line_history 并传递 CWD 给它，以便它能调用 utils::print_prompt_sec
        let input = read_line_history(
            &mut history,
            &mut history_index,
            &current_working_directory // <--- 传递 CWD
        );
        let trimmed = input.trim();

        if !trimmed.is_empty() {
            // 避免重复添加完全相同的命令到历史记录
            if history.last().map_or(true, |last_cmd| last_cmd != trimmed) {
                 history.push(trimmed.to_string());
            }
            history_index = Some(history.len()); // 重置到最新（指向新命令之后的位置）
        }

        let line: Vec<&str> = trimmed.split_whitespace().collect();
        let command = line.get(0).unwrap_or(&"");

        match command {
            &"\x04" | &"exit" => { // Ctrl+D 或 exit
                // println!(); // println! 会在 read_line_history 中处理回车时打印
                break;
            }
            &"ps" => sys_stat(),
            &"ls" => {
                let path_arg = if line.len() >= 2 { line[1] } else { "." }; // 默认为当前目录
                let path_to_list = normalize_path(&current_working_directory, path_arg);
                if let Err(e) = list_dir(&path_to_list) { // list_dir 应该在 services 或 lib 中
                    errln!("ls: {}: {}", path_to_list, e);
                }
            }
            &"cat" => {
                if line.len() < 2 {
                    println!("Usage: cat <file>");
                    continue;
                }
                let file_path_arg = line[1];
                let absolute_file_path = normalize_path(&current_working_directory, file_path_arg);
                services::cat_file(&absolute_file_path);
            }
            &"cd" => {
                if line.len() < 2 {
                    current_working_directory = String::from("/"); // cd 到根目录
                } else {
                    let target_dir_arg = line[1];
                    let new_cwd_candidate = normalize_path(&current_working_directory, target_dir_arg);
                    
                    current_working_directory = new_cwd_candidate;
                }
            }
            &"pwd" => {
                println!("{}", current_working_directory);
            }
            &"help" => utils::show_help_text(),
            &"clear" => utils::clear_screen(),
            &"lsapp" => {
                sys_list_app();
            }
            &"exec" => {
                if line.len() < 2 {
                    println!("Usage: exec <program_name> [args...]");
                } else {
                    println!("Executing: {}", line[1]);
                    services::exec(line[1]);
                    println!("Program {} executed.", line[1]);
                }
            }
            &"kill" => {
                 if line.len() < 2 {
                    println!("Usage: kill <pid>");
                } else {
                    if let Ok(pid) = line[1].parse::<u16>() {
                        services::kill(pid);
                    } else {
                        errln!("kill: Invalid PID: {}", line[1]);
                    }
                }
            }
            other => {
                if other.is_empty() {
                    continue; // 用户只按了回车
                }
                errln!("Command not found: {}", other.bright_red());
                println!("Type 'help' to see available commands.");
            }
        }
    }
    0
}

entry!(main);