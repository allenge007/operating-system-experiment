use super::uart16550::SerialPort;
use owo_colors::OwoColorize;

const SERIAL_IO_PORT: u16 = 0x3F8; // COM1

once_mutex!(pub SERIAL: SerialPort);

pub fn init() {
    init_SERIAL(SerialPort::new(SERIAL_IO_PORT));
    get_serial_for_sure().init();
    println!("{}", crate::get_ascii_header().green().bold());
    println!("{} {}", "[+]".green().bold(), "Serial Initialized.".green());
}

guard_access_fn!(pub get_serial(SERIAL: SerialPort));

pub fn backspace() {
    get_serial_for_sure().send(8);
}
