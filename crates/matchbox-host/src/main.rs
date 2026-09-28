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

#[cfg(not(target_os = "linux"))]
fn impl_main() -> anyhow::Result<()> {
    anyhow::bail!("matchbox-host requires Linux with /dev/kvm")
}

fn main() -> anyhow::Result<()> {
    impl_main()
}
