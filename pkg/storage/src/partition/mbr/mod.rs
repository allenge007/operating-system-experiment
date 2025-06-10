//! MbrTable

mod entry;

use core::marker::PhantomData;

use crate::*;
pub use entry::*;

/// The MBR Table
///
/// The disk is a collection of partitions.
/// MBR (Master Boot Record) is the *first sector* of the disk.
/// The MBR contains information about the partitions.
///
/// [ MBR | Partitions ] [ Partition 1 ] [ Partition 2 ] [ Partition 3 ] [ Partition 4 ]
pub struct MbrTable<T, B>
where
    T: BlockDevice<B> + Clone,
    B: BlockTrait,
{
    inner: T,
    partitions: [MbrPartition; 4],
    _block: PhantomData<B>,
}

impl<T, B> PartitionTable<T, B> for MbrTable<T, B>
where
    T: BlockDevice<B> + Clone,
    B: BlockTrait,
{
    fn parse(inner: T) -> FsResult<Self> {
        let mut block = B::default();
        inner.read_block(0, &mut block)?;

        let buffer = block.as_ref();
        let mut partitions: [MbrPartition; 4] = Default::default();

        for i in 0..4 {
            // DONE: parse the mbr partition from the buffer
            //      - just ignore other fields for mbr
            let offset = 0x1BE + i * 16;
             let part_data: [u8; 16] = buffer[offset..offset+16].try_into().unwrap();

            partitions[i] = MbrPartition::parse(&part_data);
            
            let part = &partitions[i];
            if part.partition_type() != 0 {
                trace!("Partition {}: Found non-empty partition", i + 1);
                trace!("  Status: 0x{:02X} {}", 
                      part.status(),
                      if part.is_active() { "(Active/Bootable)" } else { "(Inactive)" });
                trace!("  Type: 0x{:02X} ({})", 
                      part.partition_type(), 
                      get_partition_type_name(part.partition_type()));
                trace!("  Start LBA: {}", part.begin_lba());
                trace!("  Total blocks: {}", part.total_lba());
                trace!("  Size: {} MB ({} KB)", 
                      (part.total_lba() as u64 * 512) / (1024 * 1024),
                      (part.total_lba() as u64 * 512) / 1024);
                trace!("  CHS Start: Cylinder {}, Head {}, Sector {}", 
                      part.begin_cylinder(), part.begin_head(), part.begin_sector());
                trace!("  CHS End: Cylinder {}, Head {}, Sector {}", 
                      part.end_cylinder(), part.end_head(), part.end_sector());
            }

            if partitions[i].is_active() {
                trace!("Partition {}: {:#?}", i, partitions[i]);
            }
        }

        let active_count = partitions.iter().filter(|p| p.is_active()).count();
        let total_count = partitions.iter().filter(|p| p.partition_type() != 0).count();
        trace!("=== MBR Summary ===");
        trace!("Total partitions found: {}", total_count);
        trace!("Active partitions: {}", active_count);

        Ok(Self {
            inner,
            partitions: partitions.try_into().unwrap(),
            _block: PhantomData,
        })
    }

    fn partitions(&self) -> FsResult<Vec<Partition<T, B>>> {
        let mut parts = Vec::new();
        let mut cnt = 0;
        for part in self.partitions {
            if part.is_active() {
                trace!("Creating partition object for active partition {}", cnt + 1);
                cnt += 1;
                trace!("  Partition range: LBA {} to LBA {}", 
                      part.begin_lba(), 
                      part.begin_lba() + part.total_lba() - 1);
                parts.push(Partition::new(
                    self.inner.clone(),
                    part.begin_lba() as usize,
                    part.total_lba() as usize,
                ));
            }
        }

        trace!("Created {} active partition objects", parts.len());
        Ok(parts)
    }
}

fn get_partition_type_name(partition_type: u8) -> &'static str {
    match partition_type {
        0x00 => "Empty",
        0x01 => "FAT12",
        0x04 => "FAT16 <32M",
        0x05 => "Extended",
        0x06 => "FAT16",
        0x07 => "NTFS/HPFS",
        0x0B => "Win95 FAT32",
        0x0C => "Win95 FAT32 (LBA)",
        0x0E => "Win95 FAT16 (LBA)",
        0x0F => "Win95 Extended (LBA)",
        0x82 => "Linux swap",
        0x83 => "Linux",
        0x8E => "Linux LVM",
        0xFD => "Linux RAID",
        _ => "Unknown",
    }
}