use crate::hypercalls::{self, HypercallCommand, HypercallFrame};
use crate::kvm;
use kvm_ioctls::VcpuExit;
use std::path::PathBuf;

pub fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let jail = match std::env::var("MATCHBOX_JAIL") {
        Ok(path) => PathBuf::from(path),
        Err(_) => std::env::current_dir()?.join("jail"),
    };
    std::fs::create_dir_all(&jail)?;
    tracing::info!("jail root: {}", jail.display());

    let hypervisor = kvm::Hypervisor::new()?;

    // Long mode, identity-mapped. Write CMD_EXIT into the hypercall frame at
    // GPA 0x20000000, then trap on port 0x3F0.
    let guest_code: &[u8] = &[
        0x48, 0xb8, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, // mov rax, 0x20000000
        0xc7, 0x00, 0x05, 0x00, 0x00, 0x00,                         // mov dword [rax], 5
        0xb8, 0x05, 0x00, 0x00, 0x00,                               // mov eax, 5 (CMD_EXIT)
        0xba, 0xf0, 0x03, 0x00, 0x00,                               // mov edx, 0x3F0
        0xef,                                                       // out dx, eax
        0xf4,                                                       // hlt
        0xeb, 0xfc,                                                 // jmp -4
    ];

    let mut vm = hypervisor.create_vm(0, 0x400000, guest_code)?;
    tracing::info!("VM-0 running...");

    loop {
        match vm.vcpu_fd.run()? {
            VcpuExit::Hlt => {
                tracing::info!("Guest HLT");
                break;
            }
            VcpuExit::Shutdown => {
                tracing::info!("Guest shutdown");
                break;
            }
            VcpuExit::IoOut(port, data) => {
                if port == 0x3F0 {
                    let mem_ptr = vm.mem;
                    let mem_size = vm.mem_size;
                    let frame = unsafe { &mut *vm.hc_frame };
                    if frame.command_id == 0 && data.len() >= 4 {
                        frame.command_id = u32::from_le_bytes(data[..4].try_into().unwrap());
                    }
                    tracing::info!(
                        "Hypercall: cmd={:#x} args=[{:#x},{:#x},{:#x},{:#x}]",
                        frame.command_id, frame.arg0, frame.arg1, frame.arg2, frame.arg3
                    );
                    match HypercallFrame::decode(frame.command_id) {
                        Ok(HypercallCommand::Exit) => {
                            tracing::info!("Guest exit requested");
                            break;
                        }
                        Ok(_) => {
                            let mem = unsafe { std::slice::from_raw_parts_mut(mem_ptr, mem_size) };
                            let status = hypercalls::dispatch(frame, mem, &jail);
                            tracing::info!("Hypercall done: status={status}");
                        }
                        Err(e) => tracing::error!("{}", e),
                    }
                } else {
                    tracing::warn!("I/O port {:#x}", port);
                }
            }
            r => {
                tracing::warn!("Unexpected exit: {:?}", r);
                break;
            }
        }
    }

    Ok(())
}