use super::*;
use crate::alloc::string::ToString;

impl Fat16Impl {
    pub fn new(inner: impl BlockDevice<Block512>) -> Self {
        let mut block = Block::default();
        let block_size = Block512::size();

        inner.read_block(0, &mut block).unwrap();
        let bpb = Fat16Bpb::new(block.as_ref()).unwrap();

        trace!("Loading Fat16 Volume: {:#?}", bpb);

        // HINT: FirstDataSector = BPB_ResvdSecCnt + (BPB_NumFATs * FATSz) + RootDirSectors;
        let fat_start = bpb.reserved_sector_count() as usize;
        
        // let root_dir_size = { /* DONE: get the size of root dir from bpb */ };
        // 根目录区域大小 = (根目录条目数 * 32) / 每扇区字节数
        let root_dir_size = ((bpb.root_entries_count() as usize * 32) + 
                            (bpb.bytes_per_sector() as usize - 1)) / 
                            bpb.bytes_per_sector() as usize;
        // let first_root_dir_sector = { /* FIXME: calculate the first root dir sector */ };
        // 根目录起始扇区 = 保留扇区 + (FAT表数量 * 每FAT扇区数)
        let first_root_dir_sector = fat_start + 
                                   (bpb.fat_count() as usize * bpb.sectors_per_fat() as usize);
        
        let first_data_sector = first_root_dir_sector + root_dir_size;

        Self {
            bpb,
            inner: Box::new(inner),
            fat_start,
            first_data_sector,
            first_root_dir_sector,
        }
    }

    pub fn cluster_to_sector(&self, cluster: &Cluster) -> usize {
        match *cluster {
            Cluster::ROOT_DIR => self.first_root_dir_sector,
            Cluster(c) => {
                // DONE: calculate the first sector of the cluster
                // HINT: FirstSectorofCluster = ((N – 2) * BPB_SecPerClus) + FirstDataSector;
                if c < 2 {
                    // 簇号小于2是无效的
                    self.first_data_sector
                } else {
                    ((c - 2) as usize * self.bpb.sectors_per_cluster() as usize) + self.first_data_sector
                }
            }
        }
    }

    // DONE: YOU NEED TO IMPLEMENT THE FILE SYSTEM OPERATIONS HERE
    //      - read the FAT and get next cluster
    //      - traverse the cluster chain and read the data
    //      - parse the path
    //      - open the root directory
    //      - ...
    //      - finally, implement the FileSystem trait for Fat16 with `self.handle`

    /// 读取 FAT 表获取下一个簇
    pub fn get_next_cluster(&self, cluster: u16) -> FsResult<Option<u16>> {
        // FAT16 中每个 FAT 条目占 2 字节
        let fat_offset = cluster as usize * 2;
        let sector_offset = fat_offset / self.bpb.bytes_per_sector() as usize;
        let byte_offset = fat_offset % self.bpb.bytes_per_sector() as usize;

        let mut block = Block::default();
        self.inner.read_block(self.fat_start + sector_offset, &mut block)?;

        let fat_entry = u16::from_le_bytes([
            block.as_ref()[byte_offset],
            block.as_ref()[byte_offset + 1]
        ]);

        match fat_entry {
            0x0000 => Ok(None), // 空簇
            0xFFF8..=0xFFFF => Ok(None), // 簇链结束
            _ => Ok(Some(fat_entry)), // 下一个簇
        }
    }

    /// 读取目录条目
    pub fn read_dir_entries(&self, cluster: &Cluster) -> FsResult<Vec<DirEntry>> {
        let mut entries = Vec::new();
        let mut current_cluster = *cluster;

        loop {
            let sector = self.cluster_to_sector(&current_cluster);
            let sectors_per_cluster = match current_cluster {
                Cluster::ROOT_DIR => {
                    // 根目录的大小
                    ((self.bpb.root_entries_count() as usize * 32) + 
                     (self.bpb.bytes_per_sector() as usize - 1)) / 
                     self.bpb.bytes_per_sector() as usize
                }
                _ => self.bpb.sectors_per_cluster() as usize,
            };

            // 读取簇中的所有扇区
            for i in 0..sectors_per_cluster {
                let mut block = Block::default();
                self.inner.read_block(sector + i, &mut block)?;

                // 每个扇区可以包含多个目录条目 (512 / 32 = 16)
                for j in 0..(self.bpb.bytes_per_sector() as usize / DirEntry::LEN) {
                    let offset = j * DirEntry::LEN;
                    let entry_data = &block.as_ref()[offset..offset + DirEntry::LEN];

                    // 检查是否到达目录结束
                    if entry_data[0] == 0x00 {
                        return Ok(entries);
                    }

                    // 跳过已删除的条目
                    if entry_data[0] == 0xE5 {
                        continue;
                    }

                    // 解析目录条目
                    if let Ok(entry) = DirEntry::parse(entry_data) {
                        if entry.is_valid() && !entry.is_long_name() {
                            entries.push(entry);
                        }
                    }
                }
            }

            // 对于根目录，不需要跟随簇链
            if let Cluster::ROOT_DIR = current_cluster {
                break;
            }

            // 获取下一个簇
            if let Cluster(c) = current_cluster {
                if let Ok(Some(next_cluster)) = self.get_next_cluster(c as u16) {
                    current_cluster = Cluster(next_cluster as u32);
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(entries)
    }

    /// 在目录中查找指定名称的条目
    pub fn find_entry(&self, dir_cluster: &Cluster, name: &str) -> FsResult<Option<DirEntry>> {
        let entries = self.read_dir_entries(dir_cluster)?;
        let target_name = ShortFileName::parse(name)?;
        if name.is_empty() {
            return Ok(None); // 空名称不匹配任何条目
        }
        for entry in entries {
            if entry.filename == target_name {
                return Ok(Some(entry));
            }
        }

        Ok(None)
    }

    /// 解析路径并查找文件/目录
    pub fn find_path(&self, path: &str) -> FsResult<Option<DirEntry>> {
        let path = path.trim_start_matches('/');
        if path.is_empty() {
            return Ok(None); // 根目录没有对应的 DirEntry
        }
        // info!("Finding path: {}", path);
        let parts: Vec<&str> = path.split('/').collect();
        let mut current_cluster = Cluster::ROOT_DIR;
        let mut lenth = 0;
        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                continue; // 跳过空部分
            }
            lenth += 1;
        }
        info!("Path parts: {:?}, length: {}", parts, lenth);
        for (i, part) in parts.iter().enumerate() {
            if let Some(entry) = self.find_entry(&current_cluster, part)? {
                info!("Found entry: {:?}", entry);
                if i == lenth - 1 {
                    // 找到目标文件/目录
                    info!("Returning entry: {:?}", entry);
                    return Ok(Some(entry));
                } else {
                    // 中间路径必须是目录
                    if entry.is_directory() {
                        current_cluster = entry.cluster;
                    } else {
                        return Err(FsError::NotADirectory);
                    }
                }
            } else {
                return Ok(None); // 路径不存在
            }
        }

        Ok(None)
    }

    /// 读取文件数据
    pub fn read_file_data(&self, start_cluster: u16, offset: usize, buf: &mut [u8]) -> FsResult<usize> {
        let mut current_cluster = start_cluster;
        let mut bytes_read = 0;
        let mut file_offset = 0;
        let cluster_size = self.bpb.sectors_per_cluster() as usize * self.bpb.bytes_per_sector() as usize;

        // 跳过不需要的簇
        while file_offset + cluster_size <= offset {
            if let Ok(Some(next)) = self.get_next_cluster(current_cluster) {
                current_cluster = next;
                file_offset += cluster_size;
            } else {
                return Ok(0); // 文件结束
            }
        }

        // 开始读取数据
        while bytes_read < buf.len() {
            let sector = self.cluster_to_sector(&Cluster(current_cluster as u32));
            
            // 读取簇中的所有扇区
            for i in 0..self.bpb.sectors_per_cluster() as usize {
                if bytes_read >= buf.len() {
                    break;
                }

                let mut block = Block::default();
                self.inner.read_block(sector + i, &mut block)?;

                let sector_data = block.as_ref();
                let sector_offset_in_file = file_offset + i * self.bpb.bytes_per_sector() as usize;

                // 计算在当前扇区中的起始位置
                let start_in_sector = if offset > sector_offset_in_file {
                    offset - sector_offset_in_file
                } else {
                    0
                };

                // 计算要读取的字节数
                let bytes_to_read = core::cmp::min(
                    self.bpb.bytes_per_sector() as usize - start_in_sector,
                    buf.len() - bytes_read
                );

                if bytes_to_read > 0 {
                    buf[bytes_read..bytes_read + bytes_to_read]
                        .copy_from_slice(&sector_data[start_in_sector..start_in_sector + bytes_to_read]);
                    bytes_read += bytes_to_read;
                }
            }

            file_offset += cluster_size;

            // 获取下一个簇
            if let Ok(Some(next)) = self.get_next_cluster(current_cluster) {
                current_cluster = next;
            } else {
                break; // 文件结束
            }
        }

        Ok(bytes_read)
    }
}

impl FileSystem for Fat16 {
    fn read_dir(&self, path: &str) -> FsResult<Box<dyn Iterator<Item = Metadata> + Send>> {
        // DONE: read dir and return an iterator for all entries
        let cluster = if path == "/" || path.is_empty() {
            Cluster::ROOT_DIR
        } else {
            if let Some(entry) = self.handle.find_path(path)? {
                if entry.is_directory() {
                    entry.cluster
                } else {
                    return Err(FsError::NotADirectory);
                }
            } else {
                return Err(FsError::NotADirectory);
            }
        };

        let entries = self.handle.read_dir_entries(&cluster)?;
        let metadata_list: Vec<Metadata> = entries
            .into_iter()
            .map(|entry| {
                let entry_type = if entry.is_directory() {
                    FileType::Directory
                } else {
                    FileType::File
                };

                Metadata::new(
                    entry.filename.to_string(),
                    entry_type,
                    entry.size as usize,
                    Some(entry.created_time),
                    Some(entry.modified_time),
                    Some(entry.accessed_time),
                )
            })
            .collect();

        Ok(Box::new(metadata_list.into_iter()))
    }

    fn open_file(&self, path: &str) -> FsResult<FileHandle> {
        // DONE: open file and return a file handle
        if let Some(entry) = self.handle.find_path(path)? {
            if entry.is_file() {
               let file = File::new(self.handle.clone(), entry.clone());
                
                // 从 DirEntry 创建 Metadata
                let metadata = Metadata::new(
                    entry.filename.to_string(),
                    FileType::File,
                    entry.size as usize,
                    Some(entry.created_time),
                    Some(entry.modified_time),
                    Some(entry.accessed_time),
                );
                
                Ok(FileHandle::new(metadata, Box::new(file)))
            } else {
                Err(FsError::NotAFile)
            }
        } else {
            Err(FsError::FileNotFound)
        }
    }

    fn metadata(&self, path: &str) -> FsResult<Metadata> {
        // DONE: read metadata of the file / dir
        if path == "/" || path.is_empty() {
            // 根目录的元数据
            return Ok(Metadata::new(
                "/".to_string(),
                FileType::Directory,
                0,
                None, // 根目录通常没有创建时间
                None,
                None,
            ));
        }

        if let Some(entry) = self.handle.find_path(path)? {
            let entry_type = if entry.is_directory() {
                FileType::Directory
            } else {
                FileType::File
            };

            Ok(Metadata::new(
                entry.filename.to_string(),
                entry_type,
                entry.size as usize,
                Some(entry.created_time),
                Some(entry.modified_time),
                Some(entry.accessed_time),
            ))
        } else {
            Err(FsError::FileNotFound)
        }
    }

    fn exists(&self, path: &str) -> FsResult<bool> {
        // DONE: check if the file / dir exists
        if path == "/" || path.is_empty() {
            return Ok(true);
        }

        Ok(self.handle.find_path(path)?.is_some())
    }
}
