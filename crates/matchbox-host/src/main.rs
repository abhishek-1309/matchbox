#[cfg(target_os = "linux")]
pub mod kvm;

#[cfg(target_os = "linux")]
pub mod scheduler;

#[cfg(target_os = "linux")]
pub mod memory;

#[cfg(target_os = "linux")]
pub mod hypercalls;

#[cfg(target_os = "linux")]
pub mod security;

#[cfg(target_os = "linux")]
mod host_main;

#[cfg(target_os = "linux")]
use host_main as impl_main;

#[cfg(target_os = "macos")]
pub mod hv;

#[cfg(target_os = "macos")]
mod host_main_macos;

#[cfg(target_os = "macos")]
use host_main_macos::main as impl_main;

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn impl_main() -> anyhow::Result<()> {
    anyhow::bail!("matchbox-host requires Linux (/dev/kvm) or macOS (Hypervisor.framework)")
}

fn main() -> anyhow::Result<()> {
    impl_main()
}