use crate::kvm::Hypervisor;
use anyhow::Result;
use std::os::fd::AsRawFd;

/// The golden master memory image shared across all VM instances.
pub struct GoldenMaster {
    pub memfd: memfd::Memfd,
    pub size: usize,
}

impl GoldenMaster {
    /// Bootstraps the golden master by creating a memfd with the guest binary.
    pub fn bootstrap(_hypervisor: &Hypervisor) -> Result<Self> {
        let memfd = memfd::Memfd::create("matchbox-golden-master", memfd::MemfdOptions::default())?;

        // TODO: Load the compiled guest binary into memfd
        let size = 4096 * 1024; // 4 MB placeholder
        memfd.as_file().set_len(size as u64)?;

        Ok(Self { memfd, size })
    }
}