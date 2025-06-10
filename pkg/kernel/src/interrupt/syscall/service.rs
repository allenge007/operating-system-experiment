use core::alloc::Layout;

use crate::proc::*;
use crate::memory::*;
use crate::drivers::filesystem;
use crate::utils::resource::Resource;
use crate::proc::get_process_manager;

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

pub fn list_dir(args: &SyscallArgs) -> usize {
    let path_ptr = args.arg0 as *const u8;
    let path_len = args.arg1;

    // 参数验证
    if path_ptr.is_null() || path_len == 0 {
        warn!("list_dir: Invalid parameters (ptr: {:p}, len: {})", path_ptr, path_len);
        return 1; // 返回错误码 1
    }

    // 长度合理性检查
    if path_len > 4096 {  // 防止过长的路径
        warn!("list_dir: Path too long: {}", path_len);
        return 2; // 返回错误码 2
    }

    // 安全地从用户空间读取路径字符串
    let path_slice = unsafe {
        core::slice::from_raw_parts(path_ptr, path_len)
    };

    let path_str = match core::str::from_utf8(path_slice) {
        Ok(s) => s,
        Err(e) => {
            warn!("list_dir: Invalid UTF-8 in path: {:?}", e);
            return 3; // 返回错误码 3
        }
    };

    trace!("list_dir: Listing directory '{}'", path_str);

    // 调用文件系统的 ls 函数
    filesystem::ls(path_str);
    
    0 // 成功返回 0
}

/// 打开文件
/// path: &str (arg0 as *const u8, arg1 as len) -> fd: u8 (or -1 on error)
pub fn sys_open(args: &SyscallArgs) -> usize {
    let path_ptr = args.arg0 as *const u8;
    let path_len = args.arg1;

    // 参数验证
    if path_ptr.is_null() || path_len == 0 {
        warn!("sys_open: Invalid parameters");
        return usize::MAX; // 返回 -1（转换为 usize）
    }

    if path_len > 4096 {
        warn!("sys_open: Path too long: {}", path_len);
        return usize::MAX;
    }

    // 安全地从用户空间读取路径字符串
    let path_slice = unsafe {
        core::slice::from_raw_parts(path_ptr, path_len)
    };

    let path_str = match core::str::from_utf8(path_slice) {
        Ok(s) => s,
        Err(_) => {
            warn!("sys_open: Invalid UTF-8 in path");
            return usize::MAX;
        }
    };

    trace!("sys_open: Opening file '{}'", path_str);

    // 获取当前进程
    let process_arc = get_process_manager().current(); // Corrected: Use get_process_manager().current()

    // 通过文件系统打开文件
    match filesystem::get_rootfs().fs.open_file(path_str) {
        Ok(file_handle) => {
            // 将文件句柄添加到进程的资源集合中
            let fd = process_arc.write().open_resource(Resource::File(file_handle));
            trace!("sys_open: Opened file '{}' with fd {}", path_str, fd);
            fd as usize
        }
        Err(e) => {
            warn!("sys_open: Failed to open file '{}': {:?}", path_str, e);
            usize::MAX
        }
    }
}

/// 关闭文件描述符
/// fd: arg0 as u8 -> result: usize (0 = success, -1 = error)
pub fn sys_close(args: &SyscallArgs) -> usize {
    let fd = args.arg0 as u8;

    trace!("sys_close: Closing fd {}", fd);

    // 获取当前进程
    let process_arc = get_process_manager().current(); // Corrected: Use get_process_manager().current()

    // 关闭文件描述符
    if process_arc.write().close_resource(fd) {
        trace!("sys_close: Successfully closed fd {}", fd);
        0
    } else {
        warn!("sys_close: Failed to close fd {} (not found)", fd);
        usize::MAX
    }
}