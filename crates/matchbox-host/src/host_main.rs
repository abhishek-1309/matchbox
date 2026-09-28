use crate::kvm;
use kvm_ioctls::VcpuExit;

pub fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let hypervisor = kvm::Hypervisor::new()?;

    // Trivial x86-64 guest: set stack pointer, then HLT in a loop
    //   mov rsp, 0x200000    48 c7 c4 00 00 20 00
    //   hlt                   f4
    //   jmp -3 (hlt again)   eb fd
    let guest_code: &[u8] = &[
        0x48, 0xc7, 0xc4, 0x00, 0x00, 0x20, 0x00, // mov rsp, 0x200000
        0xf4,                                      // hlt
        0xeb, 0xfd,                                // jmp -3
    ];

    let mut vm = hypervisor.create_vm(0, 0x400000, guest_code)?;
    tracing::info!("VM-0 running...");

    loop {
        match vm.vcpu_fd.run()? {
            VcpuExit::Hlt => {
                tracing::info!("Guest HLT (shutdown)");
                break;
            }
            VcpuExit::Shutdown => {
                tracing::info!("Guest shutdown");
                break;
            }
            VcpuExit::IoOut { port, data, .. } => {
                tracing::warn!("I/O port {:#x}: {:?}", port, data);
            }
            r => {
                tracing::warn!("Unexpected exit: {:?}", r);
                break;
            }
        }
    }

    Ok(())
}
