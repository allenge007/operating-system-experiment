use log::{Metadata, Record};
use x86_64::instructions::interrupts;
use core::fmt::Write;
use crate::drivers::serial::get_serial;

pub fn init() {
    static LOGGER: Logger = Logger;
    log::set_logger(&LOGGER).unwrap();

    // FIXME: Configure the logger
    log::set_max_level(log::LevelFilter::Trace);

    info!("Logger Initialized.");
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
                let _ = writeln!(serial, "[{}] {}", record.level(), record.args());
            }
        });
    }

    fn flush(&self) {}
}
