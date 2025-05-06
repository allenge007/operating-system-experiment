use owo_colors::{OwoColorize, AnsiColors};
use alloc::string::String;
use lib::*;

fn clear_screen() {
    print!("\x1b[1;1H\x1b[2J");
}

/// 使用天蓝色和粉色显示 ASCII 艺术欢迎横幅
pub fn show_welcome_text() {
    clear_screen();
    let banner = r#"
 ██████╗ ██╗  ██╗        ███╗   ███╗██╗   ██╗  ██╗   ██╗███████╗██╗  ██╗
██╔═══██╗██║  ██║        ████╗ ████║╚██╗ ██╔╝  ╚██╗ ██╔╝██╔════╝██║  ██║
██║   ██║███████║        ██╔████╔██║ ╚████╔╝    ╚████╔╝ ███████╗███████║
██║   ██║██╔══██║        ██║╚██╔╝██║  ╚██╔╝      ╚██╔╝  ╚════██║██╔══██║
╚██████╔╝██║  ██║███████╗██║ ╚═╝ ██║   ██║███████╗██║   ███████║██║  ██║
 ╚═════╝ ╚═╝  ╚═╝╚══════╝╚═╝     ╚═╝   ╚═╝╚══════╝╚═╝   ╚══════╝╚═╝  ╚═╝
    "#;
    // 使用天蓝色显示横幅
    println!("{}", banner.bright_cyan());
    // 提示信息使用粉色
    println!(
        "{}",
        "Type 'help' to see available commands.\n".bright_magenta().bold()
    );
}

/// 输出 oh‑my‑zsh 风格的提示符
///
/// 提示符分为两行：
/// 第一行显示 "[ysos@machine] [~]"，
/// 第二行显示 "╰─➜ " 作为命令输入提示符。
pub fn print_prompt() {
    // 第一行：构建带颜色的用户、主机和路径信息，主要使用天蓝色
    let user_binding = "ysos".bright_cyan();
    let user_part = user_binding.bold();
    
    let host_binding = "machine".bright_purple();
    let host_part = host_binding.bold();
    
    let path_binding = "~".bright_cyan();
    let path_part = path_binding.bold();
    
    print!("{}", "╭─[".bright_cyan());
    print!("{}", user_part);
    print!("{}", "@".bright_cyan());
    print!("{}", host_part);
    print!("{}", "]─[".bright_cyan());
    print!("{}", path_part);
    println!("{}", "]".bright_cyan());

    // 第二行：提示符本身
    print!("{}", "╰─➜ ".bright_cyan().bold());
}

/// 以下为帮助信息的格式，供 show_help_text 使用
fn format_cmds(cate: &str, actions: &[Action]) -> String {
    let mut result = String::new();
    // 分类标题用天蓝色
    result.push_str(&format!("{}\n", cate.bright_cyan().bold()));
    for action in actions {
        // 命令名称及参数用粉色，描述使用 dim 的效果
        let action_str = match action.1 {
            Some(arg) => format!("{} {}", action.0.bright_magenta().bold(), arg.bright_magenta()),
            None => format!("{}", action.0.bright_magenta().bold()),
        };

        let real_len = action.0.len() + action.1.map_or(0, |arg| arg.len() + 1);
        let blank_left = 16 - real_len;

        result.push_str(&format!(
            "{:>width$} | {}\n",
            action_str,
            action.2.dimmed(),
            width = action_str.len() + blank_left,
        ));
    }
    result
}

const VERSION_STR: &str = concat!("oh_my_zsh v", env!("CARGO_PKG_VERSION"));

struct Action(&'static str, Option<&'static str>, &'static str);

const ACTIONS_MAP: [Action; 9] = [
    Action("help", None, "show this help"),
    Action("ps", None, "show process list"),
    Action("ls", None, "list directory"),
    Action("cd", Some("<path>"), "change directory"),
    Action("cat", Some("<file>"), "show file content"),
    Action("exec", Some("<file>"), "execute file"),
    Action("nohup", Some("<file>"), "execute file in background"),
    Action("kill", Some("<pid>"), "kill process"),
    Action("clear", None, "clear screen"),
];

const SHORTCUTS: [Action; 2] = [
    Action("Ctrl + D", None, "exit shell"),
    Action("Ctrl + C", None, "cancel current command"),
];

/// 显示帮助信息，带有版本及作者 (作者: allenge)
pub fn show_help_text() {
    println!("\n{} by {}\n", VERSION_STR.bold(), "allenge".bold());
    println!("{}", "Available Commands:".bright_cyan().bold());
    println!("  {} - Show process list", "ps".bright_magenta());
    println!("  {} - List directory", "ls".bright_magenta());
    println!("  {} <file> - Execute file", "exec".bright_magenta());
    println!("  {} <pid> - Kill process", "kill".bright_magenta());
    println!("  {} - Clear screen", "clear".bright_magenta());
    println!("  {} - Exit shell", "exit".bright_magenta());
    println!();
    println!("{}", "Shortcuts:".bright_cyan().bold());
    println!("  {} - Exit shell", "Ctrl+D".bright_magenta());
    println!("  {} - Cancel command", "Ctrl+C".bright_magenta());
}