use super::*;
use alloc::sync::Weak;
use alloc::vec::Vec;
use alloc::sync::Arc;
use crate::proc::vm::ProcessVm;
use spin::*;
use crate::humanized_size;

#[derive(Clone)]
pub struct Process {
    pid: ProcessId,
    inner: Arc<RwLock<ProcessInner>>,
}

pub struct ProcessInner {
    name: String,
    parent: Option<Weak<Process>>,
    children: Vec<Arc<Process>>,
    ticks_passed: usize,
    status: ProgramStatus,
    context: ProcessContext,
    exit_code: Option<isize>,
    proc_data: Option<ProcessData>,
    proc_vm: Option<ProcessVm>,
}

impl Process {
    #[inline]
    pub fn pid(&self) -> ProcessId {
        self.pid
    }

    #[inline]
    pub fn write(&self) -> RwLockWriteGuard<ProcessInner> {
        self.inner.write()
    }

    #[inline]
    pub fn read(&self) -> RwLockReadGuard<ProcessInner> {
        self.inner.read()
    }

    pub fn new(
        name: String,
        parent: Option<Weak<Process>>,
        proc_vm: Option<ProcessVm>,
        proc_data: Option<ProcessData>,
    ) -> Arc<Self> {
        let name = name.to_ascii_lowercase();

        // create context
        let pid = ProcessId::new();
        let proc_vm = proc_vm.unwrap_or_else(|| ProcessVm::new(PageTableContext::new()));

        let inner = ProcessInner {
            name,
            parent,
            status: ProgramStatus::Ready,
            context: ProcessContext::default(),
            ticks_passed: 0,
            exit_code: None,
            children: Vec::new(),
            proc_vm: Some(proc_vm),
            proc_data: Some(proc_data.unwrap_or_default()),
        };

        trace!("New process {}#{} created.", &inner.name, pid);

        // create process struct
        Arc::new(Self {
            pid,
            inner: Arc::new(RwLock::new(inner)),
        })
    }

    pub fn kill(&self, ret: isize) {
        let mut inner = self.inner.write();

        debug!(
            "Killing process {}#{} with ret code: {}",
            inner.name(),
            self.pid,
            ret
        );

        inner.kill(self.pid(), ret);
    }

    pub fn vfork(self: &Arc<Self>) -> Arc<Self> {
        let mut inner = self.inner.write();
        let new_inner = inner.vfork(Arc::downgrade(self));
        let pid = ProcessId::new();
        let child = Arc::new(Process {
            pid,
            inner: Arc::new(RwLock::new(new_inner)),
        });
        debug!(
            "Process {}#{} vforked to {}#{}",
            inner.name(),
            self.pid,
            child.inner.read().name(),
            child.pid
        );
        inner.context.set_rax(child.pid.0 as usize);
        inner.children.push(child.clone());
        inner.pause();
        child
    }
}

impl ProcessInner {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn children(&self) -> &[Arc<Process>] {
        &self.children
    }

    pub fn add_child(&mut self, child: Arc<Process>) {
        self.children.push(child);
    }

    pub fn remove_child(&mut self, child: ProcessId) {
        self.children.retain(|c| c.pid() != child);
    }

    pub fn set_parent(&mut self, parent: Weak<Process>) {
        self.parent = Some(parent);
    }

    pub fn parent(&self) -> Option<Arc<Process>> {
        self.parent.as_ref().and_then(|p| p.upgrade())
    }
    
    pub fn tick(&mut self) {
        self.ticks_passed += 1;
    }

    pub fn status(&self) -> ProgramStatus {
        self.status
    }

    pub fn pause(&mut self) {
        self.status = ProgramStatus::Ready;
    }

    pub fn resume(&mut self) {
        self.status = ProgramStatus::Running;
    }

    pub fn block(&mut self) {
        self.status = ProgramStatus::Blocked;
    }

    pub fn exit_code(&self) -> Option<isize> {
        self.exit_code
    }

    pub fn clone_page_table(&self) -> PageTableContext {
        self.vm().page_table.clone_level_4()
    }

    pub fn is_ready(&self) -> bool {
        self.status == ProgramStatus::Ready
    }

    pub fn vm(&self) -> &ProcessVm {
        self.proc_vm.as_ref().unwrap()
    }

    pub fn vm_mut(&mut self) -> &mut ProcessVm {
        self.proc_vm.as_mut().unwrap()
    }

    pub fn handle_page_fault(&mut self, addr: VirtAddr) -> bool {
        self.vm_mut().handle_page_fault(addr)
    }

    pub fn load_elf(&mut self, elf: &ElfFile) {
        self.vm_mut().load_elf(elf);
    }

    pub fn set_return(&mut self, ret: usize) {
        self.context.set_rax(ret);
    }
    /// Save the process's context
    /// mark the process as ready
    pub(super) fn save(&mut self, context: &ProcessContext) {
        // FIXME: save the process's context
        self.context.save(context);
        self.status = ProgramStatus::Ready;
    }

    /// Restore the process's context
    /// mark the process as running
    pub(super) fn restore(&mut self, context: &mut ProcessContext) {
        // FIXME: restore the process's context
        self.context.restore(context);
        // FIXME: restore the process's page table
        // if let Some(ref proc_vm) = self.proc_vm {
        //     proc_vm.page_table.load();
        // }
        self.vm().page_table.load();
        self.status = ProgramStatus::Running;
    }

    pub fn init_stack_frame(&mut self, entry: VirtAddr, stack_top: VirtAddr) {
        self.context.init_stack_frame(entry, stack_top)
    }

    pub fn kill(&mut self, pid: ProcessId, ret: isize) {
        let children = self.children();

        // remove self from parent, and set parent to children
        if let Some(parent) = self.parent() {
            if parent.read().exit_code().is_none() {
                parent.write().remove_child(pid);
                let weak = Arc::downgrade(&parent);
                for child in children {
                    child.write().set_parent(weak.clone());
                }
            } else {
                // parent already exited, set parent to None
                for child in children {
                    child.write().set_parent(Weak::new());
                }
            }
        }

        self.proc_vm.take();
        self.proc_data.take();
        self.exit_code = Some(ret);
        self.status = ProgramStatus::Dead;
    }
    
    pub fn vfork(&mut self, parent: Weak<Process>) -> ProcessInner{
        let proc_vm = self.vm().fork(self.children.len() as u64 + 1);
        let offset = proc_vm.stack.stack_offset(&self.vm().stack);
        let mut child_context = self.context;
        child_context.set_stack_offset(offset);
        child_context.set_rax(0);
        Self {
            name: self.name.clone(),
            parent: Some(parent),
            status: ProgramStatus::Ready,
            context: child_context,
            ticks_passed: 0,
            exit_code: None,
            children: Vec::new(),
            proc_vm: Some(proc_vm),
            proc_data: self.proc_data.clone(),
        }
    }

    pub fn brk(&mut self, addr: Option<VirtAddr>) -> Option<VirtAddr> {
        self.vm_mut().brk(addr)
    }
}

impl core::ops::Deref for Process {
    type Target = Arc<RwLock<ProcessInner>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl core::ops::Deref for ProcessInner {
    type Target = ProcessData;

    fn deref(&self) -> &Self::Target {
        self.proc_data
            .as_ref()
            .expect("Process data empty. The process may be killed.")
    }
}

impl core::ops::DerefMut for ProcessInner {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.proc_data
            .as_mut()
            .expect("Process data empty. The process may be killed.")
    }
}


impl core::fmt::Debug for Process {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let inner = self.inner.read();
        f.debug_struct("Process")
            .field("pid", &self.pid)
            .field("name", &inner.name)
            .field("parent", &inner.parent().map(|p| p.pid))
            .field("status", &inner.status)
            .field("ticks_passed", &inner.ticks_passed)
            .field("children", &inner.children.iter().map(|c| c.pid.0))
            .field("status", &inner.status)
            .field("context", &inner.context)
            .field("vm", &inner.proc_vm)
            .finish()
    }
}

impl core::fmt::Display for Process {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let inner = self.inner.read();
        let (size, unit) = inner.proc_vm.as_ref().map_or((0.0, "B"), |vm| {
            let usage = vm.memory_usage();
            humanized_size(usage)
        });

        write!(
            f,
            // 在 Process Name 和 Ticks 之间添加了内存占用的列
            " #{:<3} | #{:<3} | {:<12} | {:>5.1} {:<2} | {:<7} | {:?}",
            self.pid.0, // PID
            inner.parent().map(|p| p.pid.0).unwrap_or(0), // PPID
            inner.name,                                   // 进程名
            size,                                         // 内存大小
            unit,                                         // 内存单位
            inner.ticks_passed,                           // Ticks
            inner.status                                  // 状态
        )
    }
}
