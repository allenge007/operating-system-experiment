//! File
//!
//! reference: <https://wiki.osdev.org/FAT#Directories_on_FAT12.2F16.2F32>

use super::*;

#[derive(Debug, Clone)]
pub struct File {
    /// The current offset in the file
    offset: usize,
    /// The current cluster of this file
    current_cluster: Cluster,
    /// DirEntry of this file
    entry: DirEntry,
    /// The file system handle that contains this file
    handle: Fat16Handle,
}

impl File {
    pub fn new(handle: Fat16Handle, entry: DirEntry) -> Self {
        Self {
            offset: 0,
            current_cluster: entry.cluster,
            entry,
            handle,
        }
    }

    pub fn length(&self) -> usize {
        self.entry.size as usize
    }
    
    /// 确保当前簇指向正确的位置（基于当前偏移量）
    fn ensure_correct_cluster(&mut self) -> FsResult<()> {
        let cluster_size = self.handle.bpb.sectors_per_cluster() as usize 
                          * self.handle.bpb.bytes_per_sector() as usize;
        
        let target_cluster_index = self.offset / cluster_size;
        
        // 如果偏移量在第一个簇内，重置到起始簇
        if target_cluster_index == 0 {
            self.current_cluster = self.entry.cluster;
            return Ok(());
        }

        // 需要跟踪簇链到正确位置
        // 为了简化实现，每次都从头开始（在实际系统中应该优化这一点）
        let mut current_cluster = self.entry.cluster;
        
        for _ in 0..target_cluster_index {
            if let Cluster(c) = current_cluster {
                if c == 0 {
                    return Err(FsError::InvalidOperation);
                }
                
                match self.handle.get_next_cluster(c as u16)? {
                    Some(next_cluster) => {
                        current_cluster = Cluster(next_cluster as u32);
                    }
                    None => {
                        // 簇链提前结束
                        return Err(FsError::InvalidOperation);
                    }
                }
            } else {
                return Err(FsError::InvalidOperation);
            }
        }

        self.current_cluster = current_cluster;
        Ok(())
    }

    /// 移动到下一个簇
    fn move_to_next_cluster(&mut self) -> FsResult<bool> {
        if let Cluster(c) = self.current_cluster {
            if c == 0 {
                return Ok(false);
            }
            
            match self.handle.get_next_cluster(c as u16)? {
                Some(next_cluster) => {
                    self.current_cluster = Cluster(next_cluster as u32);
                    Ok(true)
                }
                None => {
                    // 簇链结束
                    Ok(false)
                }
            }
        } else {
            Ok(false)
        }
    }

    /// 从当前簇读取数据
    fn read_from_current_cluster(&self, offset_in_cluster: usize, buf: &mut [u8]) -> FsResult<usize> {
        let sector_size = self.handle.bpb.bytes_per_sector() as usize;
        let sectors_per_cluster = self.handle.bpb.sectors_per_cluster() as usize;
        
        // 获取簇的起始扇区
        let start_sector = self.handle.cluster_to_sector(&self.current_cluster);
        
        // 计算在簇内的扇区偏移和字节偏移
        let sector_offset_in_cluster = offset_in_cluster / sector_size;
        let byte_offset_in_sector = offset_in_cluster % sector_size;
        
        let mut bytes_read = 0;
        let mut current_sector_index = sector_offset_in_cluster;
        
        while bytes_read < buf.len() && current_sector_index < sectors_per_cluster {
            let current_sector = start_sector + current_sector_index;
            
            // 读取扇区数据
            let mut block = Block::default();
            self.handle.inner.read_block(current_sector, &mut block)?;
            let sector_data = block.as_ref();
            
            // 计算在当前扇区中的起始位置
            let start_in_sector = if current_sector_index == sector_offset_in_cluster {
                byte_offset_in_sector
            } else {
                0
            };
            
            // 计算要从当前扇区读取的字节数
            let bytes_available_in_sector = sector_size - start_in_sector;
            let bytes_to_read_from_sector = core::cmp::min(
                buf.len() - bytes_read,
                bytes_available_in_sector
            );
            
            // 复制数据到缓冲区
            buf[bytes_read..bytes_read + bytes_to_read_from_sector]
                .copy_from_slice(&sector_data[start_in_sector..start_in_sector + bytes_to_read_from_sector]);
            
            bytes_read += bytes_to_read_from_sector;
            current_sector_index += 1;
        }
        
        Ok(bytes_read)
    }
}

impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        // DONE: read file content from disk
        //      CAUTION: file length / buffer size / offset
        //
        //      - `self.offset` is the current offset in the file in bytes
        //      - use `self.handle` to read the blocks
        //      - use `self.entry` to get the file's cluster
        //      - use `self.handle.cluster_to_sector` to convert cluster to sector
        //      - update `self.offset` after reading
        //      - update `self.cluster` with FAT if necessary
                // 检查是否已经到达文件结尾
        if self.offset >= self.entry.size as usize {
            return Ok(0);
        }

        // 计算实际可读取的字节数（不能超过文件剩余大小和缓冲区大小）
        let remaining_in_file = self.entry.size as usize - self.offset;
        let bytes_to_read = core::cmp::min(buf.len(), remaining_in_file);
        
        if bytes_to_read == 0 {
            return Ok(0);
        }

        let cluster_size = self.handle.bpb.sectors_per_cluster() as usize 
                          * self.handle.bpb.bytes_per_sector() as usize;
        
        let mut bytes_read = 0;

        // 确保当前簇是正确的（基于当前偏移量）
        self.ensure_correct_cluster()?;

        while bytes_read < bytes_to_read {
            // 计算在当前簇中的偏移量
            let offset_in_cluster = self.offset % cluster_size;
            let bytes_remaining_in_cluster = cluster_size - offset_in_cluster;
            
            // 计算这次从当前簇要读取的字节数
            let chunk_size = core::cmp::min(
                bytes_to_read - bytes_read,
                bytes_remaining_in_cluster
            );

            // 从当前簇读取数据
            let bytes_read_from_cluster = self.read_from_current_cluster(
                offset_in_cluster,
                &mut buf[bytes_read..bytes_read + chunk_size]
            )?;

            bytes_read += bytes_read_from_cluster;
            self.offset += bytes_read_from_cluster;

            // 如果读取的字节数少于请求的，说明可能到达了文件结尾
            if bytes_read_from_cluster < chunk_size {
                break;
            }

            // 如果当前簇读完了，但还需要读取更多数据，移动到下一个簇
            if bytes_read < bytes_to_read && self.offset % cluster_size == 0 {
                if !self.move_to_next_cluster()? {
                    // 没有下一个簇了，文件结束
                    break;
                }
            }
        }

        Ok(bytes_read)
    }
}

// NOTE: `Seek` trait is not required for this lab
impl Seek for File {
    fn seek(&mut self, pos: SeekFrom) -> FsResult<usize> {
        unimplemented!()
    }
}

// NOTE: `Write` trait is not required for this lab
impl Write for File {
    fn write(&mut self, _buf: &[u8]) -> FsResult<usize> {
        unimplemented!()
    }

    fn flush(&mut self) -> FsResult {
        unimplemented!()
    }
}
