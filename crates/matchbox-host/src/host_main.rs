use crate::hypercalls::{HypercallCommand, HypercallFrame};
use crate::kvm;
use kvm_ioctls::VcpuExit;

pub fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let hypervisor = kvm::Hypervisor::new()?;

    // Guest: trigger hypercall with CMD_EXIT (0x05) via port 0x3F0
    let guest_code: &[u8] = &[
        0x48, 0xc7, 0xc4, 0x00, 0x00, 0x20, 0x00, // mov rsp, 0x200000
        0xb8, 0x05, 0x00, 0x00, 0x00,             // mov eax, 5 (CMD_EXIT)
        0xba, 0xf0, 0x03, 0x00, 0x00,             // mov dx, 0x3F0
        0xef,                                      // out dx, eax
        0xf4,                                      // hlt
        0xeb, 0xfc,                                // jmp -4
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
            VcpuExit::IoOut(port, _data) => {
                if port == 0x3F0 {
                    let frame = unsafe { &*vm.hc_frame };
                    tracing::info!(
                        "Hypercall: cmd={:#x} args=[{:#x},{:#x},{:#x},{:#x}]",
                        frame.command_id, frame.arg0, frame.arg1, frame.arg2, frame.arg3
                    );
                    match HypercallFrame::decode(frame.command_id) {
                        Ok(HypercallCommand::Exit) => {
                            tracing::info!("Guest exit requested");
                            break;
                        }
                        Ok(_) => tracing::warn!("Unhandled command: {:#x}", frame.command_id),
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