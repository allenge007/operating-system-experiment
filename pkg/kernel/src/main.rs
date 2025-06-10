#![no_std]
#![no_main]

use ysos::*;
use ysos_kernel as ysos;

extern crate alloc;

boot::entry_point!(kernel_main);

pub fn kernel_main(boot_info: &'static boot::BootInfo) -> ! {
    ysos::init(boot_info);
    
    // 测试 ATA 驱动和 MBR 分区表解析
    test_ata_and_mbr();
    
    ysos::wait(spawn_init());

    ysos::shutdown();
}

fn test_ata_and_mbr() {
    println!("=== Testing ATA Drive and MBR ===");
    
    // 测试 ATA 驱动
    match drivers::ata::AtaDrive::open(0, 0) {
        Some(drive) => {
            println!("✓ ATA Drive opened: {}", drive);
            test_mbr(&drive);
        }
        None => {
            println!("✗ Failed to open ATA Drive");
        }
    }
}
use storage::BlockDevice;
fn test_mbr(drive: &drivers::ata::AtaDrive) {
    use storage::partition::{PartitionTable, mbr::MbrTable};
    
    println!("--- Testing MBR Parsing ---");
    
    match MbrTable::parse(drive.clone()) {
        Ok(mbr_table) => {
            println!("✓ MBR parsed successfully");
            
            match mbr_table.partitions() {
                Ok(partitions) => {
                    println!("✓ Found {} partition(s)", partitions.len());
                    
                    for (i, partition) in partitions.iter().enumerate() {
                        if let Ok(size) = partition.block_count() {
                            let size_mb = (size * 512) / (1024 * 1024);
                            println!("  Partition {}: {} blocks ({} MB)", i + 1, size, size_mb);
                        }
                    }
                }
                Err(e) => println!("✗ Failed to get partitions: {:?}", e),
            }
        }
        Err(e) => println!("✗ Failed to parse MBR: {:?}", e),
    }
}

pub fn spawn_init() -> proc::ProcessId {
    proc::list_app();
    proc::spawn("sh").unwrap()
}