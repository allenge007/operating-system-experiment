mod context;
mod data;
mod manager;
mod paging;
mod pid;
mod process;
mod processor;
mod vm;
mod sync;

use alloc::sync::Arc;
use alloc::vec::Vec;
use manager::*;
use process::*;
use sync::*;

pub use context::ProcessContext;
pub use data::ProcessData;
pub use paging::PageTableContext;
pub use pid::ProcessId;
pub use vm::*;
pub use manager::get_process_manager;
use xmas_elf::ElfFile;

use alloc::string::{String, ToString};
use x86_64::VirtAddr;
use x86_64::structures::idt::PageFaultErrorCode;

pub const KERNEL_PID: ProcessId = ProcessId(1);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ProgramStatus {
    Running,
    Ready,
    Blocked,
    Dead,
}

/// init process manager
pub fn init(boot_info: &'static boot::BootInfo) {
    let proc_vm = ProcessVm::new(PageTableContext::new()).init_kernel_vm(&boot_info.kernel_pages);

    trace!("Init kernel vm: {:#?}", proc_vm);

    // kernel process
    let kproc = Process::new(String::from("kernel"), None, Some(proc_vm), None);

    kproc.write().resume();
    let app_list = boot_info.load_apps.as_ref();
    manager::init(kproc, app_list);

    info!("Process Manager Initialized.");
}

pub fn switch(context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = get_process_manager();
        let pid = manager.save_current(context);
        manager.push_ready(pid);
        manager.switch_next(context);
    });
}

pub fn print_process_list() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        get_process_manager().print_process_list();
    })
}

pub fn env(key: &str) -> Option<String> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        get_process_manager().current().read().env(key)
    })
}

pub fn process_exit(ret: isize, context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = get_process_manager();
        //DONE: implement process exit
        manager.kill_self(ret);
        manager.switch_next(context);
    })
}

pub fn wait_pid(pid: ProcessId, context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = get_process_manager();
        if let Some(ret) = manager.get_exit_code(pid) {
            context.set_rax(ret as usize);
        } else {
            manager.wait_pid(pid);
            manager.save_current(context);
            manager.current().write().block();
            manager.switch_next(context);
        }
    })
}

pub(crate) fn wait_no_block(pid: ProcessId) -> Option<isize> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        get_process_manager().get_exit_code(pid)
    })
}

pub fn read(fd: u8, buf: &mut [u8]) -> isize {
    x86_64::instructions::interrupts::without_interrupts(|| get_process_manager().read(fd, buf))
}

pub fn write(fd: u8, buf: &[u8]) -> isize {
    x86_64::instructions::interrupts::without_interrupts(|| get_process_manager().write(fd, buf))
}

pub fn get_pid() -> ProcessId {
    x86_64::instructions::interrupts::without_interrupts(processor::get_pid)
}

pub fn kill(pid: ProcessId, context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = get_process_manager();
        if pid == processor::get_pid() {
            manager.kill_self(0xdead);
            manager.switch_next(context);
        } else {
            manager.kill(pid, 0xdead);
        }
    })
}

pub fn spawn(name: &str) -> Result<ProcessId, String> {
    let app = x86_64::instructions::interrupts::without_interrupts(|| {
        let app_list = get_process_manager().app_list()?;

        app_list.iter().find(|&app| app.name.eq(name))
    });

    if app.is_none() {
        return Err(format!("App not found: {}", name));
    };

    elf_spawn(name.to_string(), &app.unwrap().elf)
}

pub fn elf_spawn(name: String, elf: &ElfFile) -> Result<ProcessId, String> {
    let pid = x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = get_process_manager();
        let process_name = name.to_lowercase();

        let parent = Arc::downgrade(&manager.current());

        let pid = manager.spawn(elf, name, Some(parent), None);

        debug!("Spawned process: {}#{}", process_name, pid);
        pid
    });

    Ok(pid)
}

pub fn current_proc_info() {
    debug!("{:#?}", get_process_manager().current())
}

pub fn handle_page_fault(addr: VirtAddr, err_code: PageFaultErrorCode) -> bool {
    x86_64::instructions::interrupts::without_interrupts(|| {
        get_process_manager().handle_page_fault(addr, err_code)
    })
}

pub fn list_app() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let app_list = get_process_manager().app_list();
        if app_list.is_none() {
            println!(">>> No app found in list!");
            return;
        }

        let apps = app_list
            .unwrap()
            .iter()
            .map(|app| app.name.as_str())
            .collect::<Vec<&str>>()
            .join(", ");

        println!(">>> App list: {}", apps);
    });
}

pub fn vfork(context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = get_process_manager();
        let pid = manager.save_current(context);
        manager.vfork();
        manager.push_ready(pid);
        manager.switch_next(context);
    })
}

pub fn sem_new(key: u32, value: usize) -> usize {
    x86_64::instructions::interrupts::without_interrupts(|| {
        if get_process_manager().current().write().sem_new(key, value) {
            return 0;
        }
        1
    })
}

pub fn remove_sem(key: u32) -> usize {
    x86_64::instructions::interrupts::without_interrupts(|| {
        if get_process_manager().current().write().sem_remove(key) {
            return 0;
        }
        1
    })
}

pub fn sem_wait(key: u32, context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = get_process_manager();
        let pid = manager.current().pid();
        let result = manager.current().write().sem_wait(key, pid);
        match result {
            SemaphoreResult::Ok => context.set_rax(0),
            SemaphoreResult::Block(pid) => {
                manager.save_current(context);
                manager.block(pid);
                manager.switch_next(context);
            },
            SemaphoreResult::NotExist => context.set_rax(1),
            _ => unreachable!(),
        }
    })
}

pub fn sem_signal(key: u32, context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = get_process_manager();
        let result = manager.current().write().sem_signal(key);
        match result {
            SemaphoreResult::Ok => context.set_rax(0),
            SemaphoreResult::WakeUp(pid) => manager.wake_up(pid, None),
            SemaphoreResult::NotExist => context.set_rax(1),
            _ => unreachable!(),
        }
    })
}

pub fn brk(addr: Option<VirtAddr>) -> Option<VirtAddr> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        // NOTE: `brk` does not need to get write lock
        get_process_manager().current().read().vm().brk(addr)
    })
}