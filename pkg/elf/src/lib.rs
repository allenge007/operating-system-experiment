#![no_std]

#[macro_use]
extern crate log;
extern crate alloc;

use core::ptr::{copy_nonoverlapping, write_bytes};

use alloc::vec::Vec;
use x86_64::structures::paging::page::{PageRange, PageRangeInclusive};
use x86_64::structures::paging::{mapper::*, *};
use x86_64::{PhysAddr, VirtAddr, align_up};
use xmas_elf::{ElfFile, program};

/// Map physical memory [0, max_addr)
///
/// to virtual space [offset, offset + max_addr)
pub fn map_physical_memory(
    offset: u64,
    max_addr: u64,
    page_table: &mut impl Mapper<Size2MiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    trace!("Mapping physical memory...");
    let start_frame = PhysFrame::containing_address(PhysAddr::new(0));
    let end_frame = PhysFrame::containing_address(PhysAddr::new(max_addr));

    for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
        let page = Page::containing_address(VirtAddr::new(frame.start_address().as_u64() + offset));
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            page_table
                .map_to(page, frame, flags, frame_allocator)
                .expect("Failed to map physical memory")
                .flush();
        }
    }
}

/// Map ELF file
///
/// for each segment, map current frame and set page table
pub fn map_elf(
    elf: &ElfFile,
    page_table: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    trace!("Mapping ELF file...{:?}", elf.input.as_ptr());
    let start = PhysAddr::new(elf.input.as_ptr() as u64);
    for segment in elf.program_iter() {
        map_segment(&segment, start, page_table, frame_allocator)?;
    }
    Ok(())
}

/// Unmap ELF file
pub fn unmap_elf(elf: &ElfFile, page_table: &mut impl Mapper<Size4KiB>) -> Result<(), UnmapError> {
    trace!("Unmapping ELF file...");
    let kernel_start = PhysAddr::new(elf.input.as_ptr() as u64);
    for segment in elf.program_iter() {
        unmap_segment(&segment, kernel_start, page_table)?;
    }
    Ok(())
}

/// map a range of memory
pub fn map_pages(
    addr: u64,
    pages: u64,
    page_table: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    user_access: bool,
) -> Result<PageRange, MapToError<Size4KiB>> {
    debug_assert!(pages > 0, "pages must be greater than 0");
    let range_start = Page::containing_address(VirtAddr::new(addr));
    let range_end = range_start + pages;

    map_range(
        Page::range_inclusive(range_start, range_end - 1),
        page_table,
        frame_allocator,
        user_access,
    )?;

    trace!(
        "Map hint: {:#x} -> {:#x}",
        addr,
        page_table
            .translate_page(range_start)
            .unwrap()
            .start_address()
    );

    Ok(Page::range(range_start, range_end))
}

pub fn map_range(
    page_range: PageRangeInclusive,
    page_table: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    user_access: bool,
) -> Result<(), MapToError<Size4KiB>> {
    trace!(
        "Map Range: {:#x} - {:#x} ({})",
        page_range.start.start_address().as_u64(),
        page_range.end.start_address().as_u64(),
        page_range.count()
    );

    let mut flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

    if user_access {
        flags |= PageTableFlags::USER_ACCESSIBLE;
    }

    trace!("Flags: {:?}", flags);

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        unsafe {
            page_table
                .map_to(page, frame, flags, frame_allocator)?
                .flush();
        }
    }

    Ok(())
}

/// map a range of memory
pub fn unmap_pages(
    addr: u64,
    pages: u64,
    page_table: &mut impl Mapper<Size4KiB>,
    frame_deallocator: &mut impl FrameDeallocator<Size4KiB>,
    do_dealloc: bool,
) -> Result<(), UnmapError> {
    debug_assert!(pages > 0, "pages must be greater than 0");
    let start = Page::containing_address(VirtAddr::new(addr));
    let end = start + pages - 1;

    unmap_range(
        Page::range_inclusive(start, end),
        page_table,
        frame_deallocator,
        do_dealloc,
    )
}

pub fn unmap_range(
    page_range: PageRangeInclusive,
    page_table: &mut impl Mapper<Size4KiB>,
    frame_deallocator: &mut impl FrameDeallocator<Size4KiB>,
    do_dealloc: bool,
) -> Result<(), UnmapError> {
    trace!(
        "Unmap Range: {:#x} - {:#x} ({})",
        page_range.start.start_address().as_u64(),
        page_range.end.start_address().as_u64(),
        page_range.count()
    );

    for page in page_range {
        let (frame, flush) = page_table.unmap(page)?;
        if do_dealloc {
            unsafe {
                frame_deallocator.deallocate_frame(frame);
            }
        }
        flush.flush();
    }

    Ok(())
}

fn map_segment(
    segment: &program::ProgramHeader,
    start: PhysAddr,
    page_table: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    if segment.get_type().unwrap() != program::Type::Load {
        return Ok(());
    }

    trace!("Mapping segment: {:#x?}", segment);
    let mem_size = segment.mem_size();
    let file_size = segment.file_size();
    let file_offset = segment.offset() & !0xfff;
    let phys_start_addr = start + file_offset;
    let virt_start_addr = VirtAddr::new(segment.virtual_addr());

    let start_page: Page = Page::containing_address(virt_start_addr);
    let start_frame = PhysFrame::containing_address(phys_start_addr);
    let end_frame = PhysFrame::containing_address(phys_start_addr + file_size - 1u64);

    let flags = segment.flags();
    let mut page_table_flags = PageTableFlags::PRESENT;

    if !flags.is_execute() {
        page_table_flags |= PageTableFlags::NO_EXECUTE;
    }

    if flags.is_write() {
        page_table_flags |= PageTableFlags::WRITABLE;
    }

    trace!("Segment page table flag: {:?}", page_table_flags);
    // DONT MAP ADDR DIRECTLY, ALLOCATE THEN COPY DATA
    for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
        let offset = frame - start_frame;
        let page = start_page + offset;
        unsafe {
            page_table
                .map_to(page, frame, page_table_flags, frame_allocator)?
                .flush();
        }
    }

    if mem_size > file_size {
        // .bss section (or similar), which needs to be zeroed
        let zero_start = virt_start_addr + file_size;
        let zero_end = virt_start_addr + mem_size;
        if zero_start.as_u64() & 0xfff != 0 {
            // A part of the last mapped frame needs to be zeroed. This is
            // not possible since it could already contains parts of the next
            // segment. Thus, we need to copy it before zeroing.

            let new_frame = frame_allocator
                .allocate_frame()
                .ok_or(MapToError::FrameAllocationFailed)?;

            type PageArray = [u64; Size4KiB::SIZE as usize / 8];

            let last_page = Page::containing_address(virt_start_addr + file_size - 1u64);
            let last_page_ptr = end_frame.start_address().as_u64() as *mut PageArray;
            let temp_page_ptr = new_frame.start_address().as_u64() as *mut PageArray;

            unsafe {
                // copy contents
                temp_page_ptr.write(last_page_ptr.read());
            }

            // remap last page
            if let Err(e) = page_table.unmap(last_page) {
                return Err(match e {
                    UnmapError::ParentEntryHugePage => MapToError::ParentEntryHugePage,
                    UnmapError::PageNotMapped => unreachable!(),
                    UnmapError::InvalidFrameAddress(_) => unreachable!(),
                });
            }

            unsafe {
                page_table
                    .map_to(last_page, new_frame, page_table_flags, frame_allocator)?
                    .flush();
            }
        }

        // Map additional frames.
        let start_page: Page =
            Page::containing_address(VirtAddr::new(align_up(zero_start.as_u64(), Size4KiB::SIZE)));
        let end_page = Page::containing_address(zero_end);
        for page in Page::range_inclusive(start_page, end_page) {
            let frame = frame_allocator
                .allocate_frame()
                .ok_or(MapToError::FrameAllocationFailed)?;
            unsafe {
                page_table
                    .map_to(page, frame, page_table_flags, frame_allocator)?
                    .flush();
            }
        }

        // zero bss
        unsafe {
            core::ptr::write_bytes(
                zero_start.as_mut_ptr::<u8>(),
                0,
                (mem_size - file_size) as usize,
            );
        }
    }
    Ok(())
}

/// Load & Map ELF file
///
/// for each segment, load code to new frame and set page table
pub fn load_elf(
    elf: &ElfFile,
    physical_offset: u64,
    page_table: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    user_access: bool,
) -> Result<(Vec<PageRangeInclusive>, u64), MapToError<Size4KiB>> {
    trace!("Loading ELF file...{:?}", elf.input.as_ptr());
    let mut loaded_page_ranges: Vec<PageRangeInclusive> = Vec::new();
    let mut total_usage_bytes: u64 = 0;

    for segment in elf.program_iter() {
        if segment.get_type().unwrap() != program::Type::Load {
            continue;
        }

        if segment.mem_size() == 0 {
            continue;
        }

        match load_segment(
            elf,
            physical_offset,
            &segment,
            page_table,
            frame_allocator,
            user_access,
        ) {
            Ok((page_range, usage)) => {
                if page_range.count() > 0 {
                    loaded_page_ranges.push(page_range);
                }
                total_usage_bytes += usage;
            }
            Err(e) => return Err(e),
        }
    }
    Ok((loaded_page_ranges, total_usage_bytes))
}

// Load segments to new allocated frames.
// Returns the page range mapped for this segment and its memory usage in bytes.
fn load_segment(
    elf: &ElfFile,
    physical_offset: u64,
    segment: &program::ProgramHeader,
    page_table: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    user_access: bool,
) -> Result<(PageRangeInclusive, u64), MapToError<Size4KiB>> { // MODIFIED RETURN TYPE
    trace!("Loading & mapping segment: {:#x?}", segment);
    let mem_size = segment.mem_size();
    let file_size = segment.file_size();
    let elf_file_data_offset = segment.offset(); // Actual offset in ELF file for this segment's data
    let virt_start_addr = VirtAddr::new(segment.virtual_addr());

    let flags = segment.flags();
    let mut page_table_flags = PageTableFlags::PRESENT;

    if !flags.is_execute() {
        page_table_flags |= PageTableFlags::NO_EXECUTE;
    }
    if flags.is_write() {
        page_table_flags |= PageTableFlags::WRITABLE;
    }
    if user_access {
        page_table_flags |= PageTableFlags::USER_ACCESSIBLE;
    }

    trace!("Segment page table flag: {:?}", page_table_flags);

    let segment_virt_start_page: Page<Size4KiB> = Page::containing_address(virt_start_addr);

    // --- Load file_size portion ---
    if file_size > 0 {
        let file_data_virt_end_page = Page::containing_address(virt_start_addr + file_size - 1u64);
        let pages_for_file_data = Page::range_inclusive(segment_virt_start_page, file_data_virt_end_page);
        let segment_data_in_elf = unsafe { elf.input.as_ptr().add(elf_file_data_offset as usize) };

        for (page_idx_in_segment, page) in pages_for_file_data.enumerate() {
            let frame = frame_allocator
                .allocate_frame()
                .ok_or(MapToError::FrameAllocationFailed)?;

            let data_offset_in_elf_segment_for_this_page = page_idx_in_segment as u64 * Size4KiB::SIZE;
            let bytes_to_copy_from_elf = if file_size - data_offset_in_elf_segment_for_this_page < Size4KiB::SIZE {
                file_size - data_offset_in_elf_segment_for_this_page
            } else {
                Size4KiB::SIZE
            };

            unsafe {
                copy_nonoverlapping(
                    segment_data_in_elf.add(data_offset_in_elf_segment_for_this_page as usize),
                    (frame.start_address().as_u64() + physical_offset) as *mut u8,
                    bytes_to_copy_from_elf as usize,
                );

                if bytes_to_copy_from_elf < Size4KiB::SIZE {
                    write_bytes(
                        (frame.start_address().as_u64() + physical_offset + bytes_to_copy_from_elf) as *mut u8,
                        0,
                        (Size4KiB::SIZE - bytes_to_copy_from_elf) as usize,
                    );
                }

                page_table
                    .map_to(page, frame, page_table_flags, frame_allocator)?
                    .flush();
            }
        }
    }

    // --- Handle .bss section (mem_size > file_size) ---
    if mem_size > file_size {
        let bss_virt_start_addr = virt_start_addr + file_size;
        let bss_start_page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(align_up(bss_virt_start_addr.as_u64(), Size4KiB::SIZE)));
        let bss_end_page = Page::containing_address(virt_start_addr + mem_size - 1u64);

        if bss_start_page <= bss_end_page {
            for page in Page::range_inclusive(bss_start_page, bss_end_page) {
                // Only map and zero if the page isn't already mapped (e.g. by the file_size part, if file_size was 0)
                let map_new_frame = if file_size == 0 && page == segment_virt_start_page {
                    page_table.translate_page(page).is_err() // If file_size is 0, first page is BSS
                } else {
                    page_table.translate_page(page).is_err() // Otherwise, map if not already mapped
                };

                if map_new_frame {
                    let frame = frame_allocator
                        .allocate_frame()
                        .ok_or(MapToError::FrameAllocationFailed)?;
                    unsafe {
                        page_table
                            .map_to(page, frame, page_table_flags, frame_allocator)?
                            .flush();
                        write_bytes(
                            (frame.start_address().as_u64() + physical_offset) as *mut u8,
                            0,
                            Size4KiB::SIZE as usize,
                        );
                    }
                }
            }
        }
    }

    // Calculate the overall page range and usage based on mem_size
    // mem_size is guaranteed > 0 by the caller (load_elf)
    let overall_segment_end_page = Page::containing_address(virt_start_addr + mem_size - 1u64);
    let result_page_range = Page::range_inclusive(segment_virt_start_page, overall_segment_end_page);
    let usage_bytes = result_page_range.count() as u64 * Size4KiB::SIZE;

    Ok((result_page_range, usage_bytes))
}

fn unmap_segment(
    segment: &program::ProgramHeader,
    kernel_start: PhysAddr,
    page_table: &mut impl Mapper<Size4KiB>,
) -> Result<(), UnmapError> {
    if segment.get_type().unwrap() != program::Type::Load {
        return Ok(());
    }
    trace!("Unmapping segment: {:#x?}", segment);
    let mem_size = segment.mem_size();
    let file_size = segment.file_size();
    let file_offset = segment.offset() & !0xfff;
    let phys_start_addr = kernel_start + file_offset;
    let virt_start_addr = VirtAddr::new(segment.virtual_addr());

    let start_page: Page = Page::containing_address(virt_start_addr);
    let start_frame = PhysFrame::<Size4KiB>::containing_address(phys_start_addr);
    let end_frame = PhysFrame::containing_address(phys_start_addr + file_size - 1u64);

    let flags = segment.flags();
    let mut page_table_flags = PageTableFlags::PRESENT;
    if !flags.is_execute() {
        page_table_flags |= PageTableFlags::NO_EXECUTE
    };
    if flags.is_write() {
        page_table_flags |= PageTableFlags::WRITABLE
    };

    for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
        let offset = frame - start_frame;
        let page = start_page + offset;
        page_table.unmap(page)?.1.flush();
    }

    if mem_size > file_size {
        // .bss section (or similar), which needs to be zeroed
        let zero_start = virt_start_addr + file_size;
        let zero_end = virt_start_addr + mem_size;
        if zero_start.as_u64() & 0xfff != 0 {
            // A part of the last mapped frame needs to be zeroed. This is
            // not possible since it could already contains parts of the next
            // segment. Thus, we need to copy it before zeroing.

            let last_page = Page::containing_address(virt_start_addr + file_size - 1u64);

            page_table.unmap(last_page)?.1.flush();
        }

        // Map additional frames.
        let start_page: Page =
            Page::containing_address(VirtAddr::new(align_up(zero_start.as_u64(), Size4KiB::SIZE)));
        let end_page = Page::containing_address(zero_end);
        for page in Page::range_inclusive(start_page, end_page) {
            page_table.unmap(page)?.1.flush();
        }
    }
    Ok(())
}
