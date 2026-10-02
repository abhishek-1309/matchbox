use crate::guest_image::{self, GUEST_LOAD_GPA, GUEST_MEM_SIZE, GUEST_STACK_GPA};
use crate::kvm::Hypervisor;
use anyhow::Result;
use std::os::unix::fs::FileExt;

/// The golden master memory image shared across all VM instances.
pub struct GoldenMaster {
    pub memfd: memfd::Memfd,
    pub size: usize,
}

impl GoldenMaster {
    /// Bootstraps the golden master by loading the compiled guest at `GUEST_LOAD_GPA`.
    pub fn bootstrap(_hypervisor: &Hypervisor) -> Result<Self> {
        let image = guest_image::guest_image();
        let image_end = GUEST_LOAD_GPA as usize + image.len();
        if image_end > GUEST_STACK_GPA as usize {
            anyhow::bail!(
                "guest image ends at {image_end:#x}, which overlaps the stack at {GUEST_STACK_GPA:#x}"
            );
        }

        let memfd = memfd::Memfd::create("matchbox-golden-master", memfd::MemfdOptions::default())?;
        let file = memfd.as_file();
        file.set_len(GUEST_MEM_SIZE as u64)?;
        file.write_all_at(image, GUEST_LOAD_GPA)?;

        Ok(Self {
            memfd,
            size: GUEST_MEM_SIZE,
        })
    }
}