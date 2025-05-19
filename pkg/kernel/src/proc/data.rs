use alloc::{collections::BTreeMap, sync::Arc};
use spin::RwLock;
use crate::{resource::ResourceSet};
use super::*;

#[derive(Debug, Clone)]
pub struct ProcessData {
    // shared data
    pub(super) env: Arc<RwLock<BTreeMap<String, String>>>,
    pub(super) resources: Arc <RwLock<ResourceSet>>,
    pub(super) semaphores: Arc<RwLock<SemaphoreSet>>,
}

impl Default for ProcessData {
    fn default() -> Self {
        Self {
            env: Arc::new(RwLock::new(BTreeMap::new())),
            resources: Arc::new(RwLock::new(ResourceSet::default())),
            semaphores: Arc::new(RwLock::new(SemaphoreSet::default())),
        }
    }
}

impl ProcessData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn env(&self, key: &str) -> Option<String> {
        self.env.read().get(key).cloned()
    }

    pub fn set_env(&mut self, key: &str, val: &str) {
        self.env.write().insert(key.into(), val.into());
    }

    pub fn read(&self, fd: u8, buf: &mut [u8]) -> isize {
        self.resources.read().read(fd, buf)
    }

    pub fn write(&self, fd: u8, buf: &[u8]) -> isize {
        self.resources.read().write(fd, buf)
    }

    #[inline]
    pub fn sem_new(&mut self, key: u32, value: usize) -> bool {
        self.semaphores.write().insert(key, value)
    }
    #[inline]
    pub fn sem_remove(&mut self, key: u32) -> bool {
        self.semaphores.write().remove(key)
    }
    #[inline]
    pub fn sem_wait(&self, key: u32, pid: ProcessId) -> SemaphoreResult {
        self.semaphores.read().wait(key, pid)
    }
    #[inline]
    pub fn sem_signal(&self, key: u32) -> SemaphoreResult {
        self.semaphores.read().signal(key)
    }
}
