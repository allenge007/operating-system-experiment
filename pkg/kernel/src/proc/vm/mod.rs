use alloc::{format, vec::Vec};
use boot::KernelPages;
use x86_64::{
    structures::paging::{
        mapper::{CleanUp, UnmapError},
        page::*,
        *,
    },
    VirtAddr,
};
use xmas_elf::ElfFile;
use crate::{humanized_size, memory::*};

pub mod heap;
pub mod stack;

use self::{heap::Heap, stack::Stack};

use super::PageTableContext;

// See the documentation for the `KernelPages` type
// Ignore when you not reach this part
//
// use boot::KernelPages;

type MapperRef<'a> = &'a mut OffsetPageTable<'static>;
type FrameAllocatorRef<'a> = &'a mut BootInfoFrameAllocator;

pub struct ProcessVm {
    // page table is shared by parent and child
    pub(super) page_table: PageTableContext,

    // stack is pre-process allocated
    pub(super) stack: Stack,

    // heap is allocated by brk syscall
    pub(super) heap: Heap,

    // code is hold by the first process
    // these fields will be empty for other processes
    pub(super) code: Vec<PageRangeInclusive>,
    pub(super) code_usage: u64,
}

impl ProcessVm {
    pub fn new(page_table: PageTableContext) -> Self {
        Self {
            page_table,
            stack: Stack::empty(),
            heap: Heap::empty(),
            code: Vec::new(),
            code_usage: 0,
        }
    }


    // See the documentation for the `KernelPages` type
    // Ignore when you not reach this part

    /// Initialize kernel vm
    ///
    /// NOTE: this function should only be called by the first process
    // pub fn init_kernel_vm(mut self, pages: &KernelPages) -> Self {
    //     // FIXME: record kernel code usage
    //     self.code = /* The kernel pages */;
    //     self.code_usage = /* The kernel code usage */;

    //     self.stack = Stack::kstack();

    //     // ignore heap for kernel process as we don't manage it

    //     self
    // }

    pub fn init_kernel_vm(mut self, pages: &KernelPages) -> Self {
        // DONE: load `self.code` and `self.code_usage` from `pages`
        self.code = pages.iter().cloned().collect();
        
        let mut total_usage: u64 = 0;
        for range in self.code.iter() {
            total_usage += range.count() as u64 * Size4KiB::SIZE;
        }
        self.code_usage = total_usage;
        info!(
            "Kernel code usage: {} bytes, {} sections",
            self.code_usage,
            self.code.len()
        );
        // DONE: init kernel stack (impl the const `kstack` function)
        //        `pub const fn kstack() -> Self`
        //         use consts to init stack, same with kernel config
        self.stack = Stack::kstack();
        self
    }

    pub fn brk(&self, addr: Option<VirtAddr>) -> Option<VirtAddr> {
        self.heap.brk(
            addr,
            &mut self.page_table.mapper(),
            &mut get_frame_alloc_for_sure(),
        )
    }

    pub fn load_elf(&mut self, elf: &ElfFile) {
        let mapper = &mut self.page_table.mapper();

        let alloc = &mut *get_frame_alloc_for_sure();

        self.load_elf_code(elf, mapper, alloc);
        self.stack.init(mapper, alloc);
    }

    fn load_elf_code(&mut self, elf: &ElfFile, mapper: MapperRef, alloc: FrameAllocatorRef) {
        // DONE: make the `load_elf` function return the code pages
        // DONE: calculate code usage
        let result = elf::load_elf(elf, *PHYSICAL_OFFSET.get().unwrap(), mapper, alloc, true);
        match result {
            Ok((loaded_code_pages, total_code_usage_bytes)) => {
                self.code = loaded_code_pages;
                self.code_usage = total_code_usage_bytes;
            }
            Err(e) => {
                error!("Failed to load ELF for process: {:?}", e);
                panic!("ELF loading failed: {:?}", e);
            }
        }
    }

    pub fn fork(&self, stack_offset_count: u64) -> Self {
        let owned_page_table = self.page_table.fork();
        let mapper = &mut owned_page_table.mapper();

        let alloc = &mut *get_frame_alloc_for_sure();

        Self {
            page_table: owned_page_table,
            stack: self.stack.vfork(mapper, alloc, stack_offset_count),
            heap: self.heap.fork(),

            // do not share code info
            code: Vec::new(),
            code_usage: 0,
        }
    }

    pub fn handle_page_fault(&mut self, addr: VirtAddr) -> bool {
        let mapper = &mut self.page_table.mapper();
        let alloc = &mut *get_frame_alloc_for_sure();

        self.stack.handle_page_fault(addr, mapper, alloc)
    }

    pub(super) fn memory_usage(&self) -> u64 {
        self.stack.memory_usage() + self.heap.memory_usage() + self.code_usage
    }

    pub(super) fn clean_up(&mut self) -> Result<(), UnmapError> {
        let mapper = &mut self.page_table.mapper();
        let dealloc = &mut *get_frame_alloc_for_sure();

        // FIXME: implement the `clean_up` function for `Stack`
        self.stack.clean_up(mapper, dealloc)?;

        if self.page_table.using_count() == 1 {
            // free heap
            // FIXME: implement the `clean_up` function for `Heap`
            self.heap.clean_up(mapper, dealloc)?;

            // free code
            for page_range in self.code.iter() {
                elf::unmap_range(*page_range, mapper, dealloc, true)?;
            }

            unsafe {
                // free P1-P3
                mapper.clean_up(dealloc);

                // free P4
                dealloc.deallocate_frame(self.page_table.reg.addr);
            }
        }

        // NOTE: maybe print how many frames are recycled
        //       **you may need to add some functions to `BootInfoFrameAllocator`**
        let recycled_count_after_cleanup = dealloc.frames_recycled_count();
        info!("Frames recycled after this cleanup: {} (this is a cumulative count from allocator)", recycled_count_after_cleanup);

        Ok(())
    }
}

impl core::fmt::Debug for ProcessVm {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let (size, unit) = humanized_size(self.memory_usage());

        f.debug_struct("ProcessVm")
            .field("stack", &self.stack)
            .field("heap", &self.heap)
            .field("memory_usage", &format!("{} {}", size, unit))
            .field("page_table", &self.page_table)
            .finish()
    }
}

impl Drop for ProcessVm {
    fn drop(&mut self) {
        trace!("Dropping ProcessVm, initiating cleanup...");
        if let Err(err) = self.clean_up() {
            error!("Failed to clean up process memory: {:?}", err);
        }
        trace!("ProcessVm drop complete.");
    }
}