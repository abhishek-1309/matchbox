use crate::kvm::VcpuFd;

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

#[cfg(test)]
mod tests {
    #[test]
    fn fiber_starts_runnable() {
        let state = super::FiberState::Runnable;
        assert!(matches!(state, super::FiberState::Runnable));
    }

    #[test]
    fn fiber_blocked_on_io() {
        let state = super::FiberState::BlockedOnIo(42);
        if let super::FiberState::BlockedOnIo(token) = state {
            assert_eq!(token, 42);
        } else {
            panic!("expected BlockedOnIo");
        }
    }

    #[test]
    fn fiber_terminated() {
        let state = super::FiberState::Terminated(0);
        if let super::FiberState::Terminated(code) = state {
            assert_eq!(code, 0);
        } else {
            panic!("expected Terminated");
        }
    }
}