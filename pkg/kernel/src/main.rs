#![no_std]
#![no_main]

#[macro_use]
extern crate log;

#[macro_use]
extern crate ysos_kernel as ysos;

use core::arch::asm;
use ysos_kernel::drivers::*;
use ysos_kernel::interrupt;

boot::entry_point!(kernel_main);

pub fn kernel_main(boot_info: &'static boot::BootInfo) -> ! {
    ysos::init(boot_info);
    info!("Hello World from YatSenOS v2!");
    loop {
        // debug!("This is a debug message");
        // trace!("This is a trace message");
        // info!("This is an info message");
        // warn!("This is a warning message");
        // error!("This is an error message");
        // info!("Hello World from YatSenOS v2!");
        let input = input::get_line();

        match input.trim() {
            "exit" => break,
            _ => {
                println!("😭: command not found: {}", input);
                println!("The counter value is {}", interrupt::clock::read_counter());
            }
        }
    }
    ysos::shutdown();
}
