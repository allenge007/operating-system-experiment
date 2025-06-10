//! Directory Entry
//!
//! reference: <https://wiki.osdev.org/FAT#Directories_on_FAT12.2F16.2F32>

use crate::*;
use bitflags::bitflags;
use chrono::LocalResult::Single;
use chrono::{DateTime, TimeZone, Utc};
use core::fmt::{Debug, Display};
use core::ops::*;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct DirEntry {
    pub filename: ShortFileName,
    pub modified_time: FsTime,
    pub created_time: FsTime,
    pub accessed_time: FsTime,
    pub cluster: Cluster,
    pub attributes: Attributes,
    pub size: u32,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Cluster(pub u32);

bitflags! {
    /// File Attributes
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Attributes: u8 {
        const READ_ONLY = 0x01;
        const HIDDEN    = 0x02;
        const SYSTEM    = 0x04;
        const VOLUME_ID = 0x08;
        const DIRECTORY = 0x10;
        const ARCHIVE   = 0x20;
        const LFN       = 0x0f; // Long File Name, Not Implemented
    }
}

impl DirEntry {
    pub const LEN: usize = 0x20;

    pub fn is_valid(&self) -> bool {
        !self.filename.is_unused() && !self.filename.is_eod()
    }

    pub fn is_directory(&self) -> bool {
        self.attributes.contains(Attributes::DIRECTORY)
    }

    pub fn is_file(&self) -> bool {
        !self.is_directory() && !self.attributes.contains(Attributes::VOLUME_ID)
    }

    pub fn is_long_name(&self) -> bool {
        self.attributes.contains(Attributes::LFN)
    }

    pub fn filename(&self) -> String {
        // NOTE: ignore the long file name in FAT16 for lab
        if self.is_valid() && !self.is_long_name() {
            format!("{}", self.filename)
        } else {
            String::from("unknown")
        }
    }

    /// For Standard 8.3 format
    ///
    /// reference: https://osdev.org/FAT#Standard_8.3_format
    pub fn parse(data: &[u8]) -> FsResult<DirEntry> {
        let filename = ShortFileName::new(&data[..11]);

        // DONE: parse the rest of the fields
        //      - ensure you can pass the test
        //      - you may need `prase_datetime` function
        // 解析文件属性 (偏移 11)
        let attributes = Attributes::from_bits_truncate(data[11]);

        // 解析创建时间和日期 (偏移 14-15: 时间, 16-17: 日期)
        let created_time_raw = u16::from_le_bytes([data[14], data[15]]);
        let created_date_raw = u16::from_le_bytes([data[16], data[17]]);
        let created_time = parse_datetime(created_date_raw, created_time_raw);

        // 解析访问日期 (偏移 18-19, 只有日期没有时间)
        let accessed_date_raw = u16::from_le_bytes([data[18], data[19]]);
        let accessed_time = parse_datetime(accessed_date_raw, 0);

        // 解析簇号 (偏移 20-21: 高16位, 26-27: 低16位)
        let cluster_hi = u16::from_le_bytes([data[20], data[21]]) as u32;
        let cluster_lo = u16::from_le_bytes([data[26], data[27]]) as u32;
        let cluster = (cluster_hi << 16) | cluster_lo;

        // 解析修改时间和日期 (偏移 22-23: 时间, 24-25: 日期)
        let modified_time_raw = u16::from_le_bytes([data[22], data[23]]);
        let modified_date_raw = u16::from_le_bytes([data[24], data[25]]);
        let modified_time = parse_datetime(modified_date_raw, modified_time_raw);

        // 解析文件大小 (偏移 28-31)
        let size = u32::from_le_bytes([data[28], data[29], data[30], data[31]]);

        Ok(DirEntry {
            filename,
            modified_time,
            created_time,
            accessed_time,
            cluster: Cluster(cluster),
            attributes,
            size,
        })
    }

    pub fn as_meta(&self) -> Metadata {
        self.into()
    }
}

fn parse_datetime(date: u16, time: u16) -> FsTime {
    // DONE: parse the year, month, day, hour, min, sec from time
    // FAT 日期格式: 位 15-9: 年份(相对1980), 位 8-5: 月份, 位 4-0: 日期
    let year = ((date >> 9) & 0x7F) as i32 + 1980;
    let month = ((date >> 5) & 0x0F) as u32;
    let day = (date & 0x1F) as u32;

    // FAT 时间格式: 位 15-11: 小时, 位 10-5: 分钟, 位 4-0: 秒/2
    let hour = ((time >> 11) & 0x1F) as u32;
    let min = ((time >> 5) & 0x3F) as u32;
    let sec = ((time & 0x1F) * 2) as u32;

    // 验证日期时间的有效性
    if month == 0 || month > 12 || day == 0 || day > 31 || hour > 23 || min > 59 || sec > 59 {
        return DateTime::from_timestamp_millis(0).unwrap();
    }

    if let Single(datetime) = Utc.with_ymd_and_hms(year, month, day, hour, min, sec) {
        datetime
    } else {
        DateTime::from_timestamp_millis(0).unwrap()
    }
}

#[derive(PartialEq, Eq, Clone)]
pub struct ShortFileName {
    pub name: [u8; 8],
    pub ext: [u8; 3],
}

impl ShortFileName {
    pub fn new(buf: &[u8]) -> Self {
        Self {
            name: buf[..8].try_into().unwrap(),
            ext: buf[8..11].try_into().unwrap(),
        }
    }

    pub fn basename(&self) -> &str {
        core::str::from_utf8(&self.name).unwrap()
    }

    pub fn extension(&self) -> &str {
        core::str::from_utf8(&self.ext).unwrap()
    }

    pub fn is_eod(&self) -> bool {
        self.name[0] == 0x00 && self.ext[0] == 0x00
    }

    pub fn is_unused(&self) -> bool {
        self.name[0] == 0xE5
    }

    pub fn matches(&self, sfn: &ShortFileName) -> bool {
        self.name == sfn.name && self.ext == sfn.ext
    }

    /// Parse a short file name from a string
    pub fn parse(name: &str) -> FsResult<ShortFileName> {
        // DONE: implement the parse function
        //      use `FilenameError` and into `FsError`
        //      use different error types for following conditions:
        //
        //      - use 0x20 ' ' for right padding
        //      - check if the filename is empty
        //      - check if the name & ext are too long
        //      - period `.` means the start of the file extension
        //      - check if the period is misplaced (after 8 characters)
        //      - check if the filename contains invalid characters:
        //        [0x00..=0x1F, 0x20, 0x22, 0x2A, 0x2B, 0x2C, 0x2F, 0x3A,
        //        0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x5B, 0x5C, 0x5D, 0x7C]
        // 检查文件名是否为空
        if name.is_empty() {
            return Err(FilenameError::FilenameEmpty.into());
        }

        // 定义无效字符
        const INVALID_CHARS: &[u8] = &[
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
            0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F,
            0x20, // space
            0x22, // "
            0x2A, // *
            0x2B, // +
            0x2C, // ,
            0x2F, // /
            0x3A, // :
            0x3B, // ;
            0x3C, // <
            0x3D, // =
            0x3E, // >
            0x3F, // ?
            0x5B, // [
            0x5C, // \
            0x5D, // ]
            0x7C, // |
        ];

        // 检查无效字符
        for byte in name.bytes() {
            if INVALID_CHARS.contains(&byte) {
                return Err(FilenameError::InvalidCharacter.into());
            }
        }

        // 转换为大写
        let name_upper = name.to_uppercase();

        // 查找点号位置
        if let Some(dot_pos) = name_upper.find('.') {
            // 有扩展名的情况
            let basename = &name_upper[..dot_pos];
            let extension = &name_upper[dot_pos + 1..];

            // 检查基名长度
            if basename.len() > 8 {
                return Err(FilenameError::NameTooLong.into());
            }

            // 检查扩展名长度
            // if extension.len() > 3 {
            //     return Err(FilenameError::ExtensionTooLong.into());
            // }

            // 检查点号位置 (不能在第9个字符之后)
            if dot_pos > 8 {
                return Err(FilenameError::MisplacedPeriod.into());
            }

            // 构建文件名数组
            let mut name_array = [0x20u8; 8]; // 用空格填充
            let mut ext_array = [0x20u8; 3];  // 用空格填充

            // 复制基名
            for (i, byte) in basename.bytes().enumerate() {
                name_array[i] = byte;
            }

            // 复制扩展名
            for (i, byte) in extension.bytes().enumerate() {
                ext_array[i] = byte;
            }

            Ok(ShortFileName {
                name: name_array,
                ext: ext_array,
            })
        } else {
            // 没有扩展名的情况
            if name_upper.len() > 8 {
                return Err(FilenameError::NameTooLong.into());
            }

            let mut name_array = [0x20u8; 8]; // 用空格填充
            let ext_array = [0x20u8; 3];      // 扩展名全为空格

            // 复制基名
            for (i, byte) in name_upper.bytes().enumerate() {
                name_array[i] = byte;
            }

            Ok(ShortFileName {
                name: name_array,
                ext: ext_array,
            })
        }
    }
}

impl Debug for ShortFileName {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self)
    }
}

impl Display for ShortFileName {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        if self.ext[0] == 0x20 {
            write!(f, "{}", self.basename().trim_end())
        } else {
            write!(
                f,
                "{}.{}",
                self.basename().trim_end(),
                self.extension().trim_end()
            )
        }
    }
}

impl Cluster {
    /// Magic value indicating an invalid cluster value.
    pub const INVALID: Cluster = Cluster(0xFFFF_FFF6);
    /// Magic value indicating a bad cluster.
    pub const BAD: Cluster = Cluster(0xFFFF_FFF7);
    /// Magic value indicating a empty cluster.
    pub const EMPTY: Cluster = Cluster(0x0000_0000);
    /// Magic value indicating the cluster holding the root directory
    /// (which doesn't have a number in Fat16 as there's a reserved region).
    pub const ROOT_DIR: Cluster = Cluster(0xFFFF_FFFC);
    /// Magic value indicating that the cluster is allocated and is the final cluster for the file
    pub const END_OF_FILE: Cluster = Cluster(0xFFFF_FFFF);
}

impl Add<u32> for Cluster {
    type Output = Cluster;
    fn add(self, rhs: u32) -> Cluster {
        Cluster(self.0 + rhs)
    }
}

impl AddAssign<u32> for Cluster {
    fn add_assign(&mut self, rhs: u32) {
        self.0 += rhs;
    }
}

impl Add<Cluster> for Cluster {
    type Output = Cluster;
    fn add(self, rhs: Cluster) -> Cluster {
        Cluster(self.0 + rhs.0)
    }
}

impl AddAssign<Cluster> for Cluster {
    fn add_assign(&mut self, rhs: Cluster) {
        self.0 += rhs.0;
    }
}

impl From<&DirEntry> for Metadata {
    fn from(entry: &DirEntry) -> Metadata {
        Metadata {
            entry_type: if entry.is_directory() {
                FileType::Directory
            } else {
                FileType::File
            },
            name: entry.filename(),
            len: entry.size as usize,
            created: Some(entry.created_time),
            accessed: Some(entry.accessed_time),
            modified: Some(entry.modified_time),
        }
    }
}

impl Display for Cluster {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "0x{:08X}", self.0)
    }
}

impl Debug for Cluster {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "0x{:08X}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dir_entry() {
        let data = hex_literal::hex!(
            "4b 45 52 4e 45 4c 20 20 45 4c 46 20 00 00 0f be
             d0 50 d0 50 00 00 0f be d0 50 02 00 f0 e4 0e 00"
        );

        let res = DirEntry::parse(&data).unwrap();

        assert_eq!(&res.filename.name, b"KERNEL  ");
        assert_eq!(&res.filename.ext, b"ELF");
        assert_eq!(res.attributes, Attributes::ARCHIVE);
        assert_eq!(res.cluster, Cluster(2));
        assert_eq!(res.size, 0xee4f0);
        assert_eq!(
            res.created_time,
            Utc.with_ymd_and_hms(2020, 6, 16, 23, 48, 30).unwrap()
        );
        assert_eq!(
            res.modified_time,
            Utc.with_ymd_and_hms(2020, 6, 16, 23, 48, 30).unwrap()
        );
        assert_eq!(
            res.accessed_time,
            Utc.with_ymd_and_hms(2020, 6, 16, 0, 0, 0).unwrap()
        );

        println!("{:#?}", res);
    }
}
