use anyhow::Result;
use kvm_bindings::*;
use kvm_ioctls::{Kvm, VmFd, VcpuFd};
use std::os::unix::io::AsRawFd;

pub struct Hypervisor {
    pub kvm: Kvm,
}

pub struct VmInstance {
    pub vm_fd: VmFd,
    pub vcpu_fd: VcpuFd,
    pub vm_id: u32,
    pub mem_size: usize,
    pub mem: *mut u8,
    pub hc_frame: *mut HypercallFrame,
}

use crate::hypercalls::HypercallFrame;

// Guest physical address of the hypercall frame
const HYPERCALL_GPA: u64 = 0x2000_0000;

impl Hypervisor {
    pub fn new() -> Result<Self> {
        let kvm = Kvm::new()?;
        tracing::info!("KVM v{}", kvm.get_api_version());
        Ok(Self { kvm })
    }

    pub fn create_vm(
        &self,
        vm_id: u32,
        mem_size: usize,
        guest_code: &[u8],
    ) -> Result<VmInstance> {
        let vm_fd = self.kvm.create_vm()?;

        // Allocate guest memory with mmap
        let mem = unsafe {
            let ptr = libc::mmap(
                std::ptr::null_mut(),
                mem_size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            );
            if ptr == libc::MAP_FAILED {
                anyhow::bail!("mmap failed: {}", std::io::Error::last_os_error());
            }
            ptr as *mut u8
        };

        // Copy guest code into guest memory at offset 0
        unsafe {
            std::ptr::copy_nonoverlapping(guest_code.as_ptr(), mem, guest_code.len());
        }

        // Set up page tables at 0x1000, 0x2000, 0x3000
        setup_page_tables(mem);

        // Register main memory slot (GPA 0)
        let mem_region = kvm_userspace_memory_region {
            slot: 0,
            flags: 0,
            guest_phys_addr: 0,
            memory_size: mem_size as u64,
            userspace_addr: mem as u64,
        };
        unsafe {
            vm_fd.set_user_memory_region(mem_region)?;
        }

        // Allocate and register hypercall frame (GPA 0x2000_0000)
        let hc_mem = unsafe {
            let ptr = libc::mmap(
                std::ptr::null_mut(),
                4096,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            );
            if ptr == libc::MAP_FAILED {
                anyhow::bail!("hc mmap failed: {}", std::io::Error::last_os_error());
            }
            std::ptr::write_bytes(ptr, 0, 4096);
            ptr as *mut u8
        };
        let hc_region = kvm_userspace_memory_region {
            slot: 1,
            flags: 0,
            guest_phys_addr: HYPERCALL_GPA,
            memory_size: 4096,
            userspace_addr: hc_mem as u64,
        };
        unsafe {
            vm_fd.set_user_memory_region(hc_region)?;
        }

        // Create vCPU and set registers
        let vcpu_fd = vm_fd.create_vcpu(0)?;

        let mut sregs = vcpu_fd.get_sregs()?;
        sregs.cr3 = 0x1000;
        sregs.cr4 |= 0x20;
        sregs.cr0 = 0x80000001;
        sregs.efer = 0x500;
        vcpu_fd.set_sregs(&sregs)?;

        let mut regs = vcpu_fd.get_regs()?;
        regs.rip = 0;
        regs.rflags = 2;
        vcpu_fd.set_regs(&regs)?;

        Ok(VmInstance {
            vm_fd,
            vcpu_fd,
            vm_id,
            mem_size,
            mem,
            hc_frame: hc_mem as *mut HypercallFrame,
        })
    }
}

impl Drop for VmInstance {
    fn drop(&mut self) {
        if !self.mem.is_null() {
            unsafe {
                libc::munmap(self.mem as *mut libc::c_void, self.mem_size);
            }
        }
        if !self.hc_frame.is_null() {
            unsafe {
                libc::munmap(self.hc_frame as *mut libc::c_void, 4096);
            }
        }
    }
}

/// Identity-map first 2MB using 4-level page tables at offsets 0x1000-0x3000.
fn setup_page_tables(mem: *mut u8) {
    // PML4 at 0x1000
    let pml4 = unsafe { &mut *(mem.add(0x1000) as *mut [u64; 512]) };
    pml4[0] = 0x2000 | 0x03;

    // PDPT at 0x2000
    let pdpt = unsafe { &mut *(mem.add(0x2000) as *mut [u64; 512]) };
    pdpt[0] = 0x3000 | 0x03;

    // Page Directory at 0x3000 (2MB pages)
    let pd = unsafe { &mut *(mem.add(0x3000) as *mut [u64; 512]) };
    for i in 0..512 {
        pd[i] = (i as u64) << 21 | 0x83;
    }
}
