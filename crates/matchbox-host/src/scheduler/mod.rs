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
        let _ = (&hypervisor, &golden_master);
        tracing::info!("scheduler jail: {}", jail_root.display());

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
    _worker_id: usize,
    local: Worker<VcpuFiber>,
    injector: Arc<Injector<VcpuFiber>>,
    _stealer: Stealer<VcpuFiber>,
) {
    loop {
        // Find a task: check local queue first, then steal
        let fiber = local.pop().or_else(|| injector.steal_batch_and_pop(&local).success());

        match fiber {
            Some(mut fiber) => {
                match fiber.state {
                    vcpu_fiber::FiberState::Runnable => {
                        // Execute guest until VM-Exit
                        // This will block the OS thread.
                        // In future: signal-based preemption.
                        if let Err(e) = drive_fiber(&mut fiber) {
                            tracing::error!("VM-{} error: {}", fiber.vm_id, e);
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

enum ExitKind {
    Hypercall,
    Hlt,
    Shutdown,
    Other,
}

fn drive_fiber(fiber: &mut VcpuFiber) -> Result<()> {
    let vm_id = fiber.vm_id;
    let kind = match fiber.vcpu_fd.run()? {
        kvm_ioctls::VcpuExit::IoOut(port, data) if port == 0x3F0 => {
            tracing::debug!("VM-{vm_id}: hypercall via port 0x3F0: {} bytes", data.len());
            ExitKind::Hypercall
        }
        kvm_ioctls::VcpuExit::Hlt => ExitKind::Hlt,
        kvm_ioctls::VcpuExit::Shutdown => ExitKind::Shutdown,
        other => {
            tracing::debug!("VM-{vm_id}: exit {other:?}");
            ExitKind::Other
        }
    };
    fiber.state = match kind {
        ExitKind::Hlt => vcpu_fiber::FiberState::Terminated(0),
        ExitKind::Shutdown => vcpu_fiber::FiberState::Terminated(-1),
        ExitKind::Hypercall | ExitKind::Other => vcpu_fiber::FiberState::Runnable,
    };
    Ok(())
}