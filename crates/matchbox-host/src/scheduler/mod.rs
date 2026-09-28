use crossbeam_deque::{Injector, Stealer, Worker};
use std::sync::Arc;

pub mod vcpu_fiber;

pub use vcpu_fiber::VcpuFiber;

use crate::kvm::Hypervisor;
use crate::memory::GoldenMaster;
use anyhow::Result;

/// The M:N work-stealing scheduler for vCPU fibers.
pub struct Scheduler;

impl Scheduler {
    pub fn run(
        hypervisor: Hypervisor,
        golden_master: GoldenMaster,
        jail_root: std::path::PathBuf,
    ) -> Result<()> {
        let injector = Arc::new(Injector::<VcpuFiber>::new());
        let num_workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .max(2);

        let mut handles = Vec::new();

        for worker_id in 0..num_workers {
            let injector = Arc::clone(&injector);
            let worker = Worker::new_fifo();
            let stealer: Stealer<VcpuFiber> = worker.stealer();

            let handle = std::thread::Builder::new()
                .name(format!("matchbox-worker-{}", worker_id))
                .spawn(move || {
                    worker_loop(worker_id, worker, injector, stealer);
                })?;

            handles.push(handle);
        }

        // Join all worker threads
        for handle in handles {
            handle.join().expect("worker thread panicked");
        }

        Ok(())
    }
}

fn worker_loop(
    worker_id: usize,
    local: Worker<VcpuFiber>,
    injector: Arc<Injector<VcpuFiber>>,
    stealers: Vec<Stealer<VcpuFiber>>,
) {
    loop {
        // Find a task: check local queue first, then steal
        let fiber = local
            .pop()
            .or_else(|| {
                // Try to steal from another worker
                // Simplified: just pop from injector
                injector.steal_batch_and_pop(&local)
            });

        match fiber {
            Some(mut fiber) => {
                match fiber.state {
                    vcpu_fiber::FiberState::Runnable => {
                        // Execute guest until VM-Exit
                        // This will block the OS thread.
                        // In future: signal-based preemption.
                        match run_vcpu(&mut fiber) {
                            Ok(exit) => handle_exit(&mut fiber, exit, &local, &injector),
                            Err(e) => {
                                tracing::error!("VM-{} error: {}", fiber.vm_id, e);
                            }
                        }
                    }
                    vcpu_fiber::FiberState::BlockedOnIo(_) => {
                        // Check if I/O is complete; if so, make Runnable
                        // For now, skip
                        continue;
                    }
                    vcpu_fiber::FiberState::Terminated(_) => {
                        // Cleanup
                        continue;
                    }
                }
            }
            None => {
                // No work available — yield
                std::thread::yield_now();
            }
        }
    }
}

fn run_vcpu(fiber: &mut VcpuFiber) -> Result<kvm_ioctls::VcpuExit> {
    use std::os::unix::io::AsRawFd;
    let ret = unsafe {
        libc::ioctl(fiber.vcpu_fd.as_raw_fd(), kvm_ioctls::KVM_RUN)
    };
    if ret != 0 {
        anyhow::bail!("KVM_RUN failed: {}", std::io::Error::last_os_error());
    }
    Ok(fiber.vcpu_fd.get_run().exit_reason)
}

fn handle_exit(
    fiber: &mut VcpuFiber,
    exit: kvm_ioctls::VcpuExit,
    _local: &Worker<VcpuFiber>,
    _injector: &Injector<VcpuFiber>,
) {
    match exit {
        kvm_ioctls::VcpuExit::IoOut { port, data, .. } => {
            if port == 0x3F0 {
                // Hypercall trap
                // Decode and handle
                tracing::debug!("VM-{}: hypercall via port 0x3F0: data={:x}", fiber.vm_id, data);
            }
            fiber.state = vcpu_fiber::FiberState::Runnable;
        }
        kvm_ioctls::VcpuExit::Hlt => {
            fiber.state = vcpu_fiber::FiberState::Terminated(0);
        }
        kvm_ioctls::VcpuExit::Shutdown => {
            fiber.state = vcpu_fiber::FiberState::Terminated(-1);
        }
        _ => {
            fiber.state = vcpu_fiber::FiberState::Runnable;
        }
    }
}