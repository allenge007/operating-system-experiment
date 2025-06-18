use syscall_def::Syscall;

#[inline(always)]
pub fn sys_write(fd: u8, buf: &[u8]) -> Option<usize> {
    let ret = syscall!(
        Syscall::Write,
        fd as u64,
        buf.as_ptr() as u64,
        buf.len() as u64
    ) as isize;
    if ret.is_negative() {
        None
    } else {
        Some(ret as usize)
    }
}

#[inline(always)]
pub fn sys_read(fd: u8, buf: &mut [u8]) -> Option<usize> {
    let ret = syscall!(
        Syscall::Read,
        fd as u64,
        buf.as_ptr() as u64,
        buf.len() as u64
    ) as isize;
    if ret.is_negative() {
        None
    } else {
        Some(ret as usize)
    }
}

#[inline(always)]
pub fn sys_wait_pid(pid: u16) -> isize {
    syscall!(Syscall::WaitPid, pid as u64) as isize
}

#[inline(always)]
pub fn sys_list_app() {
    syscall!(Syscall::ListApp);
}

#[inline(always)]
pub fn sys_stat() {
    syscall!(Syscall::Stat);
}

#[inline(always)]
pub fn sys_allocate(layout: &core::alloc::Layout) -> *mut u8 {
    syscall!(Syscall::Allocate, layout as *const _) as *mut u8
}

#[inline(always)]
pub fn sys_deallocate(ptr: *mut u8, layout: &core::alloc::Layout) -> usize {
    syscall!(Syscall::Deallocate, ptr, layout as *const _)
}

#[inline(always)]
pub fn sys_spawn(path: &str) -> u16 {
    syscall!(Syscall::Spawn, path.as_ptr() as u64, path.len() as u64) as u16
}

#[inline(always)]
pub fn sys_get_pid() -> u16 {
    syscall!(Syscall::GetPid) as u16
}

#[inline(always)]
pub fn sys_exit(code: isize) -> ! {
    syscall!(Syscall::Exit, code as u64);
    unreachable!("This process should be terminated by now.");
}

#[inline(always)]
pub fn sys_kill(pid: u16) {
    syscall!(Syscall::Kill, pid as u64);
}

#[inline(always)]
pub fn sys_fork() -> u16 {
    syscall!(Syscall::VFork) as u16
}

#[inline(always)]
pub fn sys_new_sem(key: u32, value: usize) -> bool {
    syscall!(Syscall::Sem, 0, key as u64, value) == 0
}

#[inline(always)]
pub fn sys_sem_wait(key: u32) -> bool {
    syscall!(Syscall::Sem, 1, key as u64) == 0
}

#[inline(always)]
pub fn sys_sem_signal(key: u32) -> bool {
    syscall!(Syscall::Sem, 2, key as u64) == 0
}

#[inline(always)]
pub fn sys_sem_free(key: u32) -> bool {
    syscall!(Syscall::Sem, 3, key as u64) == 0
}

#[inline(always)]
pub fn list_dir(path: &str) -> Result<(), &'static str> {
    let ret = syscall!(
        Syscall::ListDir,
        path.as_ptr() as u64,
        path.len() as u64
    ) as usize;
    
    match ret {
        0 => Ok(()),
        1 => Err("Null path pointer"),
        2 => Err("Empty path"),
        3 => Err("Path too long"),
        4 => Err("Invalid UTF-8 in path"),
        5 => Err("Empty path string"),
        _ => Err("Unknown error"),
    }
}

/// 打开文件
#[inline(always)]
pub fn open(path: &str) -> Result<u8, &'static str> {
    let ret = syscall!(
        Syscall::Open,
        path.as_ptr() as u64,
        path.len() as u64
    ) as usize;
    
    if ret == usize::MAX {
        Err("Failed to open file")
    } else {
        Ok(ret as u8)
    }
}

/// 关闭文件描述符
#[inline(always)]
pub fn close(fd: u8) -> Result<(), &'static str> {
    let ret = syscall!(Syscall::Close, fd as u64) as usize;
    
    if ret == 0 {
        Ok(())
    } else {
        Err("Failed to close file")
    }
}

/// 从文件描述符读取数据
#[inline(always)]
pub fn read(fd: u8, buf: &mut [u8]) -> isize {
    syscall!(
        Syscall::Read,
        fd as u64,
        buf.as_mut_ptr() as u64,
        buf.len() as u64
    ) as isize
}

#[inline(always)]
pub fn sys_brk(addr: Option<usize>) -> Option<usize> {
    const BRK_FAILED: usize = !0;
    match syscall!(Syscall::Brk, addr.unwrap_or(0)) {
        BRK_FAILED => None,
        ret => Some(ret),
    }
}