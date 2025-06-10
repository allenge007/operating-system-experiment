use crate::drivers::input::*;
use alloc::{collections::BTreeMap, string::String, vec::Vec};
use spin::Mutex;
use storage::common::FileHandle;

#[derive(Debug, Clone)]
pub enum StdIO {
    Stdin,
    Stdout,
    Stderr,
}

#[derive(Debug)]
pub struct ResourceSet {
    pub handles: BTreeMap<u8, Mutex<Resource>>,
    recycled: Vec<u8>,
}

impl Default for ResourceSet {
    fn default() -> Self {
        let mut res = Self {
            handles: BTreeMap::new(),
            recycled: Vec::new(),
        };

        res.open(Resource::Console(StdIO::Stdin));
        res.open(Resource::Console(StdIO::Stdout));
        res.open(Resource::Console(StdIO::Stderr));

        res
    }
}

impl ResourceSet {
    pub fn open(&mut self, res: Resource) -> u8 {
        let fd = match self.recycled.pop() {
            Some(fd) => fd,
            None => self.handles.len() as u8,
        };
        self.handles.insert(fd, Mutex::new(res));
        fd
    }

    pub fn close(&mut self, fd: u8) -> bool {
        match self.handles.remove(&fd) {
            Some(_) => {
                self.recycled.push(fd);
                true
            }
            None => false,
        }
    }

    pub fn read(&self, fd: u8, buf: &mut [u8]) -> isize {
        match self.handles.get(&fd).and_then(|h| h.lock().read(buf)) {
            Some(count) => count as isize,
            None => -1,
        }
    }

    pub fn write(&self, fd: u8, buf: &[u8]) -> isize {
        match self.handles.get(&fd).and_then(|h| h.lock().write(buf)) {
            Some(count) => count as isize,
            None => -1,
        }
    }
}

pub enum Resource {
    Console(StdIO),
    File(FileHandle),
    Null,
}

impl Resource {
    pub fn read(&mut self, buf: &mut [u8]) -> Option<usize> {
        match self {
            Resource::Console(stdio) => match stdio {
                &mut StdIO::Stdin => {
                    // just read from kernel input buffer
                    if let Some(ch) = try_pop_key() {
                        buf[0] = ch;
                        Some(1)
                    } else {
                        Some(0)
                    }
                }
                _ => None,
            },
            Resource::File(file_handle) => {
                // 从文件读取数据
                match file_handle.read(buf) {
                    Ok(bytes_read) => Some(bytes_read),
                    Err(e) => {
                        warn!("File read error: {:?}", e);
                        None
                    }
                }
            },
            Resource::Null => Some(0),
        }
    }

    pub fn write(&mut self, buf: &[u8]) -> Option<usize> {
        match self {
            Resource::Console(stdio) => match *stdio {
                StdIO::Stdin => None,
                StdIO::Stdout => {
                    print!("{}", String::from_utf8_lossy(buf));
                    Some(buf.len())
                }
                StdIO::Stderr => {
                    warn!("{}", String::from_utf8_lossy(buf));
                    Some(buf.len())
                }
            },
            Resource::File(_file_handle) => {
                // 文件写入暂不实现，直接忽略
                warn!("File write not implemented");
                None
            },
            Resource::Null => Some(buf.len()),
        }
    }
}

impl core::fmt::Debug for Resource {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Resource::Console(stdio) => write!(f, "Console({:?})", stdio),
            Resource::File(file_handle) => write!(f, "File({:?})", file_handle), // Added this arm
            Resource::Null => write!(f, "Null"),
        }
    }
}
