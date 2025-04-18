use super::*;
use crate::memory::{
    self,
    allocator::{ALLOCATOR, HEAP_SIZE},
    get_frame_alloc_for_sure, PAGE_SIZE,
};
use alloc::{collections::*, format};
use alloc::sync::Arc;
use spin::{Mutex, RwLock};
use uefi::proto::debug;

pub static PROCESS_MANAGER: spin::Once<ProcessManager> = spin::Once::new();

pub fn init(init: Arc<Process>) {

    // FIXME: set init process as Running
    init.write().resume();
    // FIXME: set processor's current pid to init's pid
    processor::set_pid(init.pid());

    PROCESS_MANAGER.call_once(|| ProcessManager::new(init));
}

pub fn get_process_manager() -> &'static ProcessManager {
    PROCESS_MANAGER
        .get()
        .expect("Process Manager has not been initialized")
}

pub struct ProcessManager {
    processes: RwLock<BTreeMap<ProcessId, Arc<Process>>>,
    ready_queue: Mutex<VecDeque<ProcessId>>,
}

impl ProcessManager {
    pub fn new(init: Arc<Process>) -> Self {
        let mut processes = BTreeMap::new();
        let ready_queue = VecDeque::new();
        let pid = init.pid();

        trace!("Init {:#?}", init);

        processes.insert(pid, init);
        Self {
            processes: RwLock::new(processes),
            ready_queue: Mutex::new(ready_queue),
        }
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

    pub fn current(&self) -> Arc<Process> {
        self.get_proc(&processor::get_pid())
            .expect("No current process")
    }

    pub fn save_current(&self, context: &ProcessContext) {
        // FIXME: update current process's tick count
        let current_pid = processor::get_pid();
        if let Some(proc) = self.get_proc(&current_pid) {
            proc.write().tick();
            // FIXME: save current process's context
            proc.write().save(context);
        }
    }

    pub fn switch_next(&self, context: &mut ProcessContext) -> ProcessId {

        // FIXME: fetch the next process from ready queue
        // FIXME: check if the next process is ready,
        //        continue to fetch if not ready
        let mut pid = processor::get_pid();

        while let Some(next) = self.ready_queue.lock().pop_front() {
            let map = self.processes.read();
            let proc = map.get(&next).expect("Process not found");

            if !proc.read().is_ready() {
                debug!("Process #{} is {:?}", next, proc.read().status());
                continue;
            }

            if pid != next {
                // FIXME: restore next process's context
                proc.write().restore(context);
                // debug!("Switch to process #{}", next);
                // FIXME: update processor's current pid
                processor::set_pid(next);
                pid = next;
            }

            break;
        }
        // print_process_list();
        // FIXME: return next process's pid
        pid
    }

    pub fn spawn_kernel_thread(
        &self,
        entry: VirtAddr,
        name: String,
        proc_data: Option<ProcessData>,
    ) -> ProcessId {
        let kproc = self.get_proc(&KERNEL_PID).unwrap();
        let page_table = kproc.read().clone_page_table();
        let proc_vm = Some(ProcessVm::new(page_table));
        let proc = Process::new(name, Some(Arc::downgrade(&kproc)), proc_vm, proc_data);
        
        debug!("Spawn kernel thread: {:#?}", proc);
        // alloc stack for the new process base on pid
        let stack_top = proc.alloc_init_stack();
        
        // FIXME: set the stack frame
        // let mut init_context = ProcessContext::default();
        // init_context.init_stack_frame(entry, stack_top);
        // proc.write().save(&init_context);

        proc.write().init_stack_frame(entry, stack_top);
        // FIXME: push to ready queue
        let pid = proc.pid();
        self.add_proc(pid, proc);
        self.push_ready(pid);
        debug!("Process #{} created.", pid);
        // debug!("Ready Queue: {:?}", self.ready_queue.lock());
        // FIXME: return new process pid
        pid
    }

    pub fn kill_current(&self, ret: isize) {
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
        let proc = self.get_proc(&pid);

        if proc.is_none() {
            warn!("Process #{} not found.", pid);
            return;
        }

        let proc = proc.unwrap();

        if proc.read().status() == ProgramStatus::Dead {
            warn!("Process #{} is already dead.", pid);
            return;
        }

        trace!("Kill {:#?}", &proc);

        proc.kill(ret);
    }

    pub fn print_process_list(&self) {
        let mut output = String::from("  PID | PPID | Process Name |  Ticks  | Status\n");

        self.processes
            .read()
            .values()
            .filter(|p| p.read().status() != ProgramStatus::Dead)
            .for_each(|p| output += format!("{}\n", p).as_str());

        // TODO: print memory usage of kernel heap

        output += format!("Queue  : {:?}\n", self.ready_queue.lock()).as_str();

        output += &processor::print_processors();

        print!("{}", output);
    }
}
