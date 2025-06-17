use super::*;
use alloc::{format, collections::BTreeMap, collections::BTreeSet, collections::VecDeque, sync::Weak};
use spin::{Mutex, RwLock};
use crate::memory::{PAGE_SIZE, get_frame_alloc_for_sure, FRAME_ALLOCATOR}; // 确保导入 FRAME_ALLOCATOR 和 PAGE_SIZE
use crate::humanized_size; // 确保导入 humanized_size 函数

pub static PROCESS_MANAGER: spin::Once<ProcessManager> = spin::Once::new();

pub fn init(init: Arc<Process>, app_list: boot::AppListRef) {
    // FIXME: set init process as Running
    // FIXME: set processor's current pid to init's pid
    processor::set_pid(init.pid());
    PROCESS_MANAGER.call_once(|| ProcessManager::new(init, app_list));
}

pub fn get_process_manager() -> &'static ProcessManager {
    PROCESS_MANAGER
        .get()
        .expect("Process Manager has not been initialized")
}

fn format_system_memory_usage(name: &str, used_bytes: u64, total_bytes: u64) -> String {
    if total_bytes == 0 {
        return format!(
            "{:<6} : {:>6} {:>3} / {:>6} {:>3} (  N/A  %)\n", // 中文冒号
            name,
            humanized_size(used_bytes as u64).0,
            humanized_size(used_bytes as u64).1,
            humanized_size(total_bytes as u64).0,
            humanized_size(total_bytes as u64).1
        );
    }
    let (used_float, used_unit) = humanized_size(used_bytes as u64);
    let (total_float, total_unit) = humanized_size(total_bytes as u64);

    format!(
        "{:<6} ：{:>6.*} {:>3} / {:>6.*} {:>3} ({:>5.2}%)\n", // 中文冒号
        name,
        2, // 小数点后两位
        used_float,
        used_unit,
        2, // 小数点后两位
        total_float,
        total_unit,
        used_bytes as f32 / total_bytes as f32 * 100.0
    )
}

pub struct ProcessManager {
    processes: RwLock<BTreeMap<ProcessId, Arc<Process>>>,
    ready_queue: Mutex<VecDeque<ProcessId>>,
    app_list: boot::AppListRef,
    wait_queue: Mutex<BTreeMap<ProcessId, BTreeSet<ProcessId>>>,
}

impl ProcessManager {
    pub fn new(init: Arc<Process>, app_list: boot::AppListRef) -> Self {
        let mut processes = BTreeMap::new();
        let ready_queue = VecDeque::new();
        let pid = init.pid();

        trace!("Init {:#?}", init);

        processes.insert(pid, init);
        Self {
            processes: RwLock::new(processes),
            ready_queue: Mutex::new(ready_queue),
            app_list: app_list,
            wait_queue: Mutex::new(BTreeMap::new()),
        }
    }
    #[inline]
    pub fn app_list(&self) -> boot::AppListRef {
        self.app_list
    }

    #[inline]
    pub fn push_ready(&self, pid: ProcessId) {
        self.ready_queue.lock().push_back(pid);
    }

    #[inline]
    fn add_proc(&self, pid: ProcessId, proc: Arc<Process>) {
        self.processes.write().insert(pid, proc);
    }

    #[inline]
    pub fn get_proc(&self, pid: &ProcessId) -> Option<Arc<Process>> {
        self.processes.read().get(pid).cloned()
    }

    pub(super) fn get_exit_code(&self, pid: ProcessId) -> Option<isize> {
        self.get_proc(&pid).and_then(|p| p.read().exit_code())
    }

    pub fn current(&self) -> Arc<Process> {
        self.get_proc(&processor::get_pid())
            .expect("No current process")
    }

    pub fn wait_pid(&self, pid: ProcessId) {
        let mut wait_queue = self.wait_queue.lock();
        let entry = wait_queue.entry(pid).or_default();
        entry.insert(processor::get_pid());
    }

    #[inline]
    pub fn read(&self, fd: u8, buf: &mut [u8]) -> isize {
        self.current().read().read(fd, buf)
    }

    #[inline]
    pub fn write(&self, fd: u8, buf: &[u8]) -> isize {
        self.current().read().write(fd, buf)
    }

    pub fn block(&self, pid: ProcessId) {
        if let Some(proc) = self.get_proc(&pid) {
            proc.write().block();
        }
    }

    pub fn save_current(&self, context: &ProcessContext) -> ProcessId {
        let cur = self.current();
        let pid = cur.pid();
        {
            let mut cur_w = cur.write();
            cur_w.tick();
            cur_w.save(context);
        }
        pid
    }

    pub fn switch_next(&self, context: &mut ProcessContext) -> ProcessId {
        let mut pid = processor::get_pid();
        let mut ready_q = self.ready_queue.lock();
        while let Some(next) = ready_q.pop_front() {
            let proc = {
                let map = self.processes.read();
                map.get(&next).expect("Process not found").clone()
            };
            if !proc.read().is_ready() {
                debug!("Process #{} is {:?}", next, proc.read().status());
                continue;
            }
            if pid != next {
                proc.write().restore(context);
                processor::set_pid(next);
                pid = next;
            }
            break;
        }
        pid
    }

    pub fn spawn(
        &self,
        elf: &ElfFile,
        name: String,
        parent: Option<Weak<Process>>,
        proc_data: Option<ProcessData>,
    ) -> ProcessId {
        let kproc = self.get_proc(&KERNEL_PID).unwrap();
        let page_table = kproc.read().clone_page_table();
        let proc_vm = Some(ProcessVm::new(page_table));
        let proc = Process::new(name, parent, proc_vm, proc_data);
        {
            let mut proc_w = proc.write();
            proc_w.pause();
            proc_w.load_elf(elf);
            proc_w.init_stack_frame(
                VirtAddr::new_truncate(elf.header.pt2.entry_point()),
                VirtAddr::new_truncate(super::stack::STACK_INIT_TOP),
            );
        }
        trace!("New {:#?}", &proc);
        let pid = proc.pid();
        self.add_proc(pid, proc);
        self.push_ready(pid);
        pid
    }

    pub fn kill_self(&self, ret: isize) {
        self.kill(processor::get_pid(), ret);
    }

    pub fn handle_page_fault(&self, addr: VirtAddr, err_code: PageFaultErrorCode) -> bool {
        // FIXME: handle page fault
        if !err_code.contains(PageFaultErrorCode::PROTECTION_VIOLATION) {
            let cur_proc = self.current();
            trace!(
                "Page Fault! Checking if {:#x} is on current process's stack",
                addr
            );

            if cur_proc.pid() == KERNEL_PID {
                info!("Page Fault on Kernel at {:#x}", addr);
            }

            let mut inner = cur_proc.write();
            inner.handle_page_fault(addr)
        } else {
            false
        }
    }

    pub fn kill(&self, pid: ProcessId, ret: isize) {
        match self.get_proc(&pid) {
            Some(proc) => {
                trace!("Kill {:#?}", &proc);
                proc.kill(ret);
                if let Some(waiters) = self.wait_queue.lock().remove(&pid) {
                    for waiter in waiters {
                        self.wake_up(waiter, Some(ret));
                    }
                }
            }
            None => {
                warn!("Process #{} not found.", pid);
            }
        }
    }

    pub fn vfork(&self) {
        let child = self.current().vfork();
        let pid = child.pid();
        self.add_proc(pid, child);
        self.push_ready(pid);
        debug!("Current queue: {:?}", self.ready_queue.lock());
    }

    pub fn wake_up(&self, pid: ProcessId, ret: Option<isize>) {
       if let Some(proc) = self.get_proc(&pid) {
            let mut inner = proc.write();
            if let Some(ret) = ret {
                inner.set_return(ret as usize);
            }
            inner.pause();
            self.push_ready(pid);
       } 
    }

    pub fn print_process_list(&self) {
        let mut output = String::from("  PID | PPID | ProcesName       | MemoryUsage |  Ticks  | Status\n"); // 修改表头为中文，并调整列名

        self.processes
            .read()
            .values()
            .filter(|p| p.read().status() != ProgramStatus::Dead)
            .for_each(|p| output += &format!("{}\n", p));

        if let Some(alloc_mutex) = FRAME_ALLOCATOR.get() {
            let alloc = alloc_mutex.lock();
            let frames_used = alloc.frames_used();
            let frames_recycled = alloc.frames_recycled_count();
            let frames_total = alloc.frames_total();

            // (已用帧 - 已回收帧) * 页大小
            let active_system_frames = frames_used.saturating_sub(frames_recycled);
            let used_mem_bytes = active_system_frames as u64 * PAGE_SIZE;
            let total_mem_bytes = frames_total as u64 * PAGE_SIZE;
            
            output += &format_system_memory_usage("SystemMemory", used_mem_bytes, total_mem_bytes);
        } else {
            output += "SystemMemory: frame allocator do not init\n";
        }

        output += &format!("ready_queue：{:?}\n", self.ready_queue.lock()); // 中文标签

        output += &processor::print_processors(); // 假设这个函数存在

        print!("{}", output); // 最终打印
    }
}
