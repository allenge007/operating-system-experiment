use core::ptr::copy_nonoverlapping;

use x86_64::{
    VirtAddr,
    structures::paging::{Page, mapper::MapToError, page::*},
};

use crate::proc::{KERNEL_PID, processor};

use super::*;

// 0xffff_ff00_0000_0000 is the kernel's address space
pub const STACK_MAX: u64 = 0x4000_0000_0000;
// stack max addr, every thread has a stack space
// from 0x????_????_0000_0000 to 0x????_????_ffff_ffff
// 0x100000000 bytes -> 4GiB
// allow 0x2000 (4096) threads run as a time
// 0x????_2000_????_???? -> 0x????_3fff_????_????
// init alloc stack has size of 0x2000 (2 frames)
// every time we meet a page fault, we alloc more frames
pub const STACK_MAX_PAGES: u64 = 0x100000;
pub const STACK_MAX_SIZE: u64 = STACK_MAX_PAGES * crate::memory::PAGE_SIZE;
pub const STACK_START_MASK: u64 = !(STACK_MAX_SIZE - 1);
// [bot..0x2000_0000_0000..top..0x3fff_ffff_ffff]
// init stack
pub const STACK_DEF_BOT: u64 = STACK_MAX - STACK_MAX_SIZE;
pub const STACK_DEF_PAGE: u64 = 1;
pub const STACK_DEF_SIZE: u64 = STACK_DEF_PAGE * crate::memory::PAGE_SIZE;

pub const STACK_INIT_BOT: u64 = STACK_MAX - STACK_DEF_SIZE;
pub const STACK_INIT_TOP: u64 = STACK_MAX - 8;

const STACK_INIT_TOP_PAGE: Page<Size4KiB> = Page::containing_address(VirtAddr::new(STACK_INIT_TOP));

// [bot..0xffffff0100000000..top..0xffffff01ffffffff]
// kernel stack
pub const KSTACK_MAX: u64 = 0xffff_ff02_0000_0000;
pub const KSTACK_DEF_BOT: u64 = KSTACK_MAX - STACK_MAX_SIZE;
pub const KSTACK_DEF_PAGE: u64 = 8;
pub const KSTACK_DEF_SIZE: u64 = KSTACK_DEF_PAGE * crate::memory::PAGE_SIZE;

pub const KSTACK_INIT_BOT: u64 = KSTACK_MAX - KSTACK_DEF_SIZE;
pub const KSTACK_INIT_TOP: u64 = KSTACK_MAX - 8;

const KSTACK_INIT_PAGE: Page<Size4KiB> = Page::containing_address(VirtAddr::new(KSTACK_INIT_BOT));
const KSTACK_INIT_TOP_PAGE: Page<Size4KiB> = Page::containing_address(VirtAddr::new(KSTACK_INIT_TOP));

pub struct Stack {
    range: PageRange<Size4KiB>,
    usage: u64,
}

impl Stack {
    pub fn new(top: Page, size: u64) -> Self {
        Self {
            range: Page::range(top - size + 1, top + 1),
            usage: size,
        }
    }

    pub const fn kstack() -> Self {
        Self {
            range: Page::range(KSTACK_INIT_PAGE, KSTACK_INIT_TOP_PAGE),
            usage: KSTACK_DEF_PAGE,
        }
    }

    pub fn empty() -> Self {
        Self {
            range: Page::range(STACK_INIT_TOP_PAGE, STACK_INIT_TOP_PAGE),
            usage: 0,
        }
    }

    pub fn memory_usage(&self) -> u64 {
        self.usage * Size4KiB::SIZE
    }
    
    pub(super) fn clean_up(
        &mut self,
        mapper: MapperRef,
        dealloc: FrameAllocatorRef,
    ) -> Result<(), UnmapError> {
        if self.usage == 0 {
            return Ok(());
        }
        // elf::unmap_pages 期望起始地址和页数
        // self.range 是 PageRange<Size4KiB>
        // PageRange 包含 start 和 end Page
        // PageRangeInclusive 包含 start 和 end Page
        let start_addr = self.range.start.start_address().as_u64();
        let page_count = self.range.count(); // PageRange::count() 返回 usize

        if page_count > 0 {
            elf::unmap_pages(start_addr, page_count as u64, mapper, dealloc, true)?;
        }

        self.usage = 0;
        Ok(())
    }

    pub fn init(&mut self, mapper: MapperRef, alloc: FrameAllocatorRef) {
        debug_assert!(self.usage == 0, "Stack is not empty.");

        self.range = elf::map_pages(STACK_INIT_BOT, STACK_DEF_PAGE, mapper, alloc, true).unwrap();
        self.usage = STACK_DEF_PAGE;
    }

    pub fn stack_offset(&self, old_stack: &Stack) -> u64 {
        let cur_stack_base = self.range.start.start_address().as_u64();
        let old_stack_base = old_stack.range.start.start_address().as_u64();
        let offset = cur_stack_base - old_stack_base;
        debug_assert!(offset % STACK_MAX_SIZE != 0, "Invalid stack offset.");
        offset
    }

    pub fn vfork(&self, mapper: MapperRef, alloc: FrameAllocatorRef, stack_offset: u64) -> Self {
        let mut new_stack_base = self.range.start.start_address().as_u64() - stack_offset * STACK_MAX_SIZE;
        while elf::map_pages(
            new_stack_base,
            self.usage,
            mapper,
            alloc,
            true,
        )
        .is_err() {
            new_stack_base -= STACK_MAX_SIZE;
        }
        debug!("Map new stack: {:#x}", new_stack_base);
        unsafe {
            copy_nonoverlapping(
                self.range.start.start_address().as_u64() as *mut u64,
                new_stack_base as *mut u64,
                (self.usage * Size4KiB::SIZE / 8) as usize,
            );
            debug!(
                "Copy stack range {:#x} to {:#x}",
                self.range.start.start_address().as_u64(),
                new_stack_base
            );
        }
        let new_start = Page::containing_address(VirtAddr::new(new_stack_base));
        let new_end = new_start + self.usage;
        Self {
            range: Page::range(new_start, new_end),
            usage: self.usage,
        }
    }

    pub fn handle_page_fault(
        &mut self,
        addr: VirtAddr,
        mapper: MapperRef,
        alloc: FrameAllocatorRef,
    ) -> bool {
        if !self.is_on_stack(addr) {
            return false;
        }

        if let Err(m) = self.grow_stack(addr, mapper, alloc) {
            error!("Grow stack failed: {:?}", m);
            return false;
        }

        true
    }

    fn is_on_stack(&self, addr: VirtAddr) -> bool {
        let addr = addr.as_u64();
        let cur_stack_bot = self.range.start.start_address().as_u64();
        trace!("Current stack bot: {:#x}", cur_stack_bot);
        trace!("Address to access: {:#x}", addr);
        addr & STACK_START_MASK == cur_stack_bot & STACK_START_MASK
    }

    fn grow_stack(
        &mut self,
        addr: VirtAddr,
        mapper: MapperRef,
        alloc: FrameAllocatorRef,
    ) -> Result<(), MapToError<Size4KiB>> {
        debug_assert!(self.is_on_stack(addr), "Address is not on stack.");

        let new_start_page = Page::containing_address(addr);
        let page_count = self.range.start - new_start_page;

        trace!(
            "Fill missing pages...[{:#x} -> {:#x}) ({} pages)",
            new_start_page.start_address().as_u64(),
            self.range.start.start_address().as_u64(),
            page_count
        );

        let user_access = processor::get_pid() != KERNEL_PID;

        elf::map_pages(
            new_start_page.start_address().as_u64(),
            page_count,
            mapper,
            alloc,
            user_access,
        )?;

        self.range = Page::range(new_start_page, self.range.end);
        self.usage = self.range.count() as u64;

        Ok(())
    }
}

impl core::fmt::Debug for Stack {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("Stack")
            .field(
                "top",
                &format_args!("{:#x}", self.range.end.start_address().as_u64()),
            )
            .field(
                "bot",
                &format_args!("{:#x}", self.range.start.start_address().as_u64()),
            )
            .finish()
    }
}
