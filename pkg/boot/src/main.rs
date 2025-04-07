#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

#[macro_use]
extern crate log;
extern crate alloc;

use alloc::boxed::Box;
use alloc::vec;
use uefi::{entry, Status};
use uefi::mem::memory_map::MemoryMap;
use elf::{load_elf, map_range, map_physical_memory};
use x86_64::structures::paging::{frame, FrameAllocator};
use xmas_elf::ElfFile;
use x86_64::registers::control::*;
use ysos_boot::*;
use ysos_boot::config::Config;

mod config;

const CONFIG_PATH: &str = "\\EFI\\BOOT\\boot.conf";
const KERNEL_PATH: &str = "\\KERNEL.ELF";

#[entry]
fn efi_main() -> Status {
    uefi::helpers::init().expect("Failed to initialize utilities");

    log::set_max_level(log::LevelFilter::Info);
    info!("Running UEFI bootloader...");

    // 1. Load config
    let mut config_file = open_file(CONFIG_PATH);
    let config = Config::parse(load_file(&mut config_file));

    info!("Config: {:#x?}", config);

    // 2. Load ELF files
    let mut kernel_file = open_file(KERNEL_PATH);
    let kernel_data = load_file(&mut kernel_file);
    let elf_file = ElfFile::new(kernel_data)
        .expect("Failed to parse ELF file");
    unsafe {
        set_entry(elf_file.header.pt2.entry_point() as usize);
    }

    // 3. Load MemoryMap
    let mmap = uefi::boot::memory_map(MemoryType::LOADER_DATA)
        .expect("Failed to get memory map");

    let max_phys_addr = mmap
        .entries()
        .map(|m| m.phys_start + m.page_count * 0x1000)
        .max()
        .unwrap()
        .max(0x1_0000_0000); // include IOAPIC MMIO area

    // 4. Map ELF segments, kernel stack and physical memory to virtual memory
    let mut page_table = current_page_table();

    // FIXME: root page table is readonly, disable write protect (Cr0)
    unsafe {
        Cr0::update(|flags| {
            flags.remove(Cr0Flags::WRITE_PROTECT);
        });
    }
    // FIXME: map physical memory to specific virtual address offset
    let mut frame_allocator = UEFIFrameAllocator;
    map_physical_memory(
        config.physical_memory_offset,
        max_phys_addr,
        &mut page_table,
        &mut frame_allocator,
    );
    // FIXME: load and map the kernel elf file
    let _ = load_elf(&elf_file, config.physical_memory_offset, &mut page_table, &mut frame_allocator)
        .expect("Failed to load ELF file");
    info!("Kernel ELF loaded");
    // FIXME: map kernel stack
    map_range(
        config.kernel_stack_address,
        config.kernel_stack_size,
        &mut page_table,
        &mut frame_allocator,
    )
    .expect("Failed to map kernel stack");
    // FIXME: recover write protect (Cr0)
    unsafe {
        Cr0::update(|flags| {
            flags.insert(Cr0Flags::WRITE_PROTECT);
        });
    }
    free_elf(elf_file);

    // 5. Pass system table to kernel
    let ptr = uefi::table::system_table_raw().expect("Failed to get system table");
    let system_table = ptr.cast::<core::ffi::c_void>();


    // 6. Exit boot and jump to ELF entry
    info!("Exiting boot services...");

    let mmap = unsafe { uefi::boot::exit_boot_services(MemoryType::LOADER_DATA) };
    // NOTE: alloc & log are no longer available

    // construct BootInfo
    let bootinfo = BootInfo {
        memory_map: mmap.entries().copied().collect(),
        physical_memory_offset: config.physical_memory_offset,
        system_table,
        log_level: "info",
    };

    // align stack to 8 bytes
    let stacktop = config.kernel_stack_address + config.kernel_stack_size * 0x1000 - 8;

    jump_to_entry(&bootinfo, stacktop);
}
