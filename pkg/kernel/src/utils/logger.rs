use log::{Level, Metadata, Record, LevelFilter};
use x86_64::instructions::interrupts;
use core::fmt::Write;
use crate::drivers::serial::get_serial;

fn level_color(level: Level) -> &'static str {
    match level {
        Level::Error => "\x1b[31m",
        Level::Warn => "\x1b[33m",
        Level::Info => "\x1b[32m",
        Level::Debug => "\x1b[34m",
        Level::Trace => "\x1b[35m",
    }
}

const RESET_COLOR: &str = "\x1b[0m";

/// 将字符串解析为 log::LevelFilter，默认返回 LevelFilter::Trace
fn parse_log_level(s: &str) -> LevelFilter {
    match s {
        "error" => LevelFilter::Error,
        "warn" | "warning" => LevelFilter::Warn,
        "info"  => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Trace,
    }
}

/// 初始化日志系统，并根据传入的字符串设置日志级别
pub fn init(log_level: &str) {
    static LOGGER: Logger = Logger;
    log::set_logger(&LOGGER).unwrap();

    // 根据启动参数设置日志级别
    let level = parse_log_level(log_level);
    log::set_max_level(level);

    info!("Logger Initialized. (log_level = {})", log_level);
}

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        // FIXME: Implement the logger with serial output
        interrupts::without_interrupts(|| {
            if let Some(mut serial) = get_serial() {
                // 直接使用固定前缀避免使用 format! 宏
                let prefix = match record.level() {
                    Level::Info  => " INFO",
                    Level::Warn  => " WARN",
                    Level::Error => "ERROR",
                    Level::Debug => "DEBUG",
                    Level::Trace => "TRACE",
                };
                let color = level_color(record.level());

                // 固定前缀宽度采用简单的右侧填空（这里假设所有前缀长度已经固定）
                if record.level() == Level::Info {
                    let _ = writeln!(
                        serial,
                        "{}[{:<5}]{} {}",
                        color,
                        prefix,
                        RESET_COLOR,
                        record.args()
                    );
                } else if record.level() == Level::Warn {
                    // WARNING：采用黄色、加粗和下划线，同时前缀宽度固定
                    let _ = writeln!(
                        serial,
                        "{}[\x1b[1m\x1b[4m{:<5}{}{}] {}{}",
                        color,
                        prefix,
                        RESET_COLOR,
                        color,
                        record.args(),
                        RESET_COLOR
                    );
                } else if record.level() == Level::Error {
                    // ERROR：采用红色和加粗，前缀固定宽度
                    let _ = writeln!(
                        serial,
                        "{}\x1b[1m[{:<5}] {}{}",
                        "\x1b[31m",
                        prefix,
                        record.args(),
                        RESET_COLOR
                    );
                } else {
                    // 对于其他日志级别直接以同样的格式输出
                    let _ = writeln!(
                        serial,
                        "{}[{:<5}]{} {}",
                        color,
                        prefix,
                        RESET_COLOR,
                        record.args()
                    );
                }
            }
        });
    }

    fn flush(&self) {}
}
