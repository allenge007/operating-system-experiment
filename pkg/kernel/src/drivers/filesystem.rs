use super::ata::*;
use alloc::boxed::Box;
use chrono::DateTime;
use storage::fat16::Fat16;
use storage::mbr::*;
use storage::*;
use alloc::string::ToString;

pub static ROOTFS: spin::Once<Mount> = spin::Once::new();

pub fn get_rootfs() -> &'static Mount {
    ROOTFS.get().unwrap()
}

pub fn init() {
    info!("Opening disk device...");

    let drive = AtaDrive::open(0, 0).expect("Failed to open disk device");

    // only get the first partition
    let part = MbrTable::parse(drive)
        .expect("Failed to parse MBR")
        .partitions()
        .expect("Failed to get partitions")
        .remove(0);

    info!("Mounting filesystem...");

    ROOTFS.call_once(|| Mount::new(Box::new(Fat16::new(part)), "/".into()));

    trace!("Root filesystem: {:#?}", ROOTFS.get().unwrap());

    info!("Initialized Filesystem.");
}

pub fn ls(root_path: &str) {
    let iter = match get_rootfs().read_dir(root_path) {
        Ok(iter) => iter,
        Err(err) => {
            warn!("{:?}", err);
            return;
        }
    };

    // DONE: format and print the file metadata
    //      - use `for meta in iter` to iterate over the entries
    //      - use `crate::humanized_size_short` for file size
    //      - add '/' to the end of directory names
    //      - format the date as you like
    //      - do not forget to print the table header
    // 打印表头
    info!("Directory listing for: {}", root_path);
    info!("{:<12} {:>10} {:>8} {:<20} {}", 
          "Type", "Size", "Name", "Modified", "Created");
    info!("{}", "-".repeat(70));

    // 遍历目录条目
    for meta in iter {
        let type_str = if meta.is_dir() { "DIR" } else { "FILE" };
        
        // 格式化文件名，目录添加 '/' 后缀
        let name_display = if meta.is_dir() {
            format!("{}/", meta.name)
        } else {
            meta.name.clone()
        };

        // 格式化文件大小
        let size_display = if meta.is_dir() {
            "-".to_string()
        } else {
           let (value, unit) = crate::humanized_size_short(meta.len as u64);
           format!("{:.1}{}", value, unit) // Format the tuple into a String
        };

        // 格式化时间
        let modified_str = match meta.modified {
            Some(time) => format!("{}", time.format("%Y-%m-%d %H:%M")),
            None => "N/A".to_string(),
        };

        let created_str = match meta.created {
            Some(time) => format!("{}", time.format("%Y-%m-%d %H:%M")),
            None => "N/A".to_string(),
        };

        info!("{:<12} {:>10} {:>8} {:<20} {}", 
              type_str, 
              size_display, 
              name_display, 
              modified_str,
              created_str);
    }
}
