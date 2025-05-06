#![no_std]
#![no_main]

use lib::*;

extern crate lib;

fn main() -> isize {
    // println!("Welcome to the Test App!");
    let mut counter = 0 as u64;
    loop{
        counter = counter + 1;
    }
    0
}

entry!(main);