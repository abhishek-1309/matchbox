// macOS backend using Apple Hypervisor.framework.

#[cfg(target_os = "macos")]
use crate::hv;

#[cfg(target_os = "macos")]
pub fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(true)
        .init();

    // ARM64 guest: load from unmapped address to force stage-2 fault exit
    //   mov x0, #0x100000000  (4GB - outside mapped 4MB)
    //   ldr x0, [x0]          (should cause stage-2 fault)
    //   b _start
    let guest_code: &[u8] = &[
        0x00, 0x00, 0x80, 0xd2, // mov x0, #0
        0x60, 0x00, 0x80, 0x92, // movk x0, #3, lsl #32  (x0 = 0x300000000)
        0x00, 0x00, 0x40, 0xf9, // ldr x0, [x0]  (fault!)
        0xfd, 0xff, 0xff, 0x17, // b _start
    ];

    let hypervisor = hv::Hypervisor::new()?;
    let mut vm = hypervisor.create_vm(0, 0x400000, guest_code)?;
    eprintln!("[test] VM created, calling hv_vcpu_run...");

    // Write a test value to the hypercall frame before running
    hv::write_hypercall_status(&vm, 42);
    eprintln!("[test] Pre-set hypercall frame status = 42");

    loop {
        match hv::run_vcpu(&mut vm)? {
            hv::VmExit::Halt => {
                eprintln!("[test] Guest HLT");
                break;
            }
            hv::VmExit::Hypercall { cmd } => {
                let frame = hv::read_hypercall_frame(&vm);
                eprintln!("[test] HVC exit: cmd={:#x} frame.status={}", cmd, frame.status);
                break; // single shot for now
            }
            hv::VmExit::Cancelled => {
                eprintln!("[test] VM cancelled");
                break;
            }
            hv::VmExit::Unknown(r) => {
                eprintln!("[test] Unexpected exit reason: {}", r);
                break;
            }
        }
    }

    eprintln!("[test] Done. status={}", hv::read_hypercall_frame(&vm).status);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn main() -> anyhow::Result<()> {
    anyhow::bail!("matchbox-host requires macOS to use Hypervisor.framework")
}