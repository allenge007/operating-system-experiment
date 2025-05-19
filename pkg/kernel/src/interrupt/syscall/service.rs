use core::alloc::Layout;

use crate::proc::*;
use crate::memory::*;

use super::SyscallArgs;

pub fn spawn_process(args: &SyscallArgs) -> usize {
    // FIXME: get app name by args
    //       - core::str::from_utf8_unchecked
    //       - core::slice::from_raw_parts
    // FIXME: spawn the process by name
    // FIXME: handle spawn error, return 0 if failed
    // FIXME: return pid as usize
    let name = unsafe {
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(
            args.arg0 as *const u8,
            args.arg1,
        ))
    };

    let pid = crate::proc::spawn(name);

    if pid.is_err() {
        warn!("spawn_process: failed to spawn process: {}", name);
        return 0;
    }

    pid.unwrap().0 as usize
}

pub fn sys_write(args: &SyscallArgs) -> usize {
    // FIXME: get buffer and fd by args
    //       - core::slice::from_raw_parts
    // FIXME: call proc::write -> isize
    // FIXME: return the result as usize
    let buf = match as_user_slice(args.arg1, args.arg2) {
        Some(buf) => buf,
        None => return usize::MAX,
    };

    let fd = args.arg0 as u8;
    write(fd, buf) as usize
}

pub fn sys_read(args: &SyscallArgs) -> usize {
    // FIXME: just like sys_write
    let buf = match as_user_slice_mut(args.arg1, args.arg2) {
        Some(buf) => buf,
        None => return usize::MAX,
    };

    let fd = args.arg0 as u8;
    read(fd, buf) as usize
}

pub fn exit_process(args: &SyscallArgs, context: &mut ProcessContext) {
    // FIXME: exit process with retcode
    process_exit(args.arg0 as isize, context);
}

pub fn list_process() {
    // FIXME: list all processes
    print_process_list();
}

pub fn sys_wait_pid(args: &SyscallArgs, context: &mut ProcessContext) {
    let pid = ProcessId(args.arg0 as u16);
    wait_pid(pid, context);
}

pub fn sys_kill(args: &SyscallArgs, context: &mut ProcessContext) {
    if args.arg0 == 1 {
        warn!("sys_kill: cannot kill kernel!");
        return;
    }

    kill(ProcessId(args.arg0 as u16), context);
}

pub fn sys_get_pid() -> u16 {
    get_pid().0
}

pub fn sys_allocate(args: &SyscallArgs) -> usize {
    let layout = unsafe { (args.arg0 as *const Layout).as_ref().unwrap() };

    if layout.size() == 0 {
        return 0;
    }

    let ret = crate::memory::user::USER_ALLOCATOR
        .lock()
        .allocate_first_fit(*layout);

    match ret {
        Ok(ptr) => ptr.as_ptr() as usize,
        Err(_) => 0,
    }
}

macro_rules! check_access {
    ($addr:expr, $fmt:expr) => {
        if !is_user_accessable($addr) {
            warn!($fmt, $addr);
            return;
        }
    };
}

pub fn sys_deallocate(args: &SyscallArgs) {
    if args.arg0 == 0 {
        return;
    }

    check_access!(args.arg0, "sys_deallocate: invalid access to {:#x}");

    check_access!(args.arg1, "sys_deallocate: invalid access to {:#x}");

    let layout = unsafe { (args.arg1 as *const Layout).as_ref().unwrap() };

    if layout.size() == 0 {
        return;
    }

    check_access!(
        args.arg0 + layout.size() - 1,
        "sys_deallocate: invalid access to {:#x}"
    );

    unsafe {
        crate::memory::user::USER_ALLOCATOR.lock().deallocate(
            core::ptr::NonNull::new_unchecked(args.arg0 as *mut u8),
            *layout,
        );
    }
}

pub fn sys_vfork(context: &mut ProcessContext) {
    vfork(context);
}

pub fn sys_sem(args: &SyscallArgs, context: &mut ProcessContext) {
    match args.arg0 {
        0 => context.set_rax(sem_new(args.arg1 as u32, args.arg2) as usize),
        1 => sem_wait(args.arg1 as u32, context),
        2 => sem_signal(args.arg1 as u32, context),
        3 => context.set_rax(remove_sem(args.arg1 as u32)),
        _ => context.set_rax(usize::MAX),
    }
}