use crate::kvm::VcpuFd;
use std::sync::Arc;

pub enum FiberState {
    Runnable,
    BlockedOnIo(u64),
    Terminated(i32),
}

pub struct VcpuFiber {
    pub vm_id: u32,
    pub vcpu_fd: VcpuFd,
    pub state: FiberState,
}