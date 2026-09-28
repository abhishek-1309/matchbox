//! Apple Hypervisor.framework bindings for macOS.
//! Single-region approach: guest code + page tables + hypercall frame in one mmap.

use anyhow::Result;

const GUEST_SIZE: usize = 0x40_0000; // 4 MB
const HYPERCALL_GPA: u64 = 0x300000; // frame at GPA 0x300000, inside guest region

#[link(name = "Hypervisor", kind = "framework")]
extern "C" {
    fn hv_vm_create(options: u64) -> i32;
    fn hv_vm_destroy() -> i32;
    fn hv_vm_map(addr: *const u8, gpa: u64, size: u64, flags: u64) -> i32;
    fn hv_vcpu_create(
        vcpu: *mut *mut core::ffi::c_void,
        exit: *mut *const HvVcpuExit,
        options: u64,
    ) -> i32;
    fn hv_vcpu_destroy(vcpu: *mut core::ffi::c_void) -> i32;
    fn hv_vcpu_run(vcpu: *mut core::ffi::c_void) -> i32;
    fn hv_vcpu_set_reg(vcpu: *mut core::ffi::c_void, reg: u32, val: u64) -> i32;
}

const HV_SUCCESS: i32 = 0;
const HV_VM_DEFAULT: u64 = 0;
const HV_VCPU_DEFAULT: u64 = 0;

const HV_MEM_READ: u64 = 1 << 0;
const HV_MEM_WRITE: u64 = 1 << 1;
const HV_MEM_EXEC: u64 = 1 << 2;

const HV_REG_PC: u32 = 32;
const HV_REG_SP: u32 = 31;
const HV_REG_CPSR: u32 = 33;

const HV_EXIT_REASON_CANCEL: u32 = 0;
const HV_EXIT_REASON_HVC: u32 = 5;
const HV_EXIT_REASON_VM_STOP: u32 = 9;

#[repr(C)]
struct HvVcpuExit {
    reason: u32,
    _reserved: u32,
    exception: HvVcpuException,
}

#[repr(C)]
struct HvVcpuException {
    syndrome: u64,
    virtual_address: u64,
    physical_address: u64,
}

pub struct Hypervisor;

pub struct VmInstance {
    pub vcpu: *mut core::ffi::c_void,
    exit_info: *const HvVcpuExit,
    pub vm_id: u32,
    mem: *mut u8,
    mem_size: usize,
}

impl Hypervisor {
    pub fn new() -> Result<Self> {
        let ret = unsafe { hv_vm_create(HV_VM_DEFAULT) };
        if ret != HV_SUCCESS {
            anyhow::bail!("hv_vm_create failed: ret={}", ret);
        }
        tracing::info!("macOS Hypervisor VM created");
        Ok(Self)
    }

    pub fn create_vm(&self, vm_id: u32, _mem_size: usize, guest_code: &[u8]) -> Result<VmInstance> {
        let mem_size = GUEST_SIZE;
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

        // Copy guest code at GPA 0
        unsafe {
            std::ptr::copy_nonoverlapping(guest_code.as_ptr(), mem, guest_code.len());
        }

        // Set up page tables at GPA 0x1000
        setup_page_tables(mem);

        // Map entire 4MB region at GPA 0
        let ret = unsafe {
            hv_vm_map(
                mem as *const u8,
                0,
                mem_size as u64,
                HV_MEM_READ | HV_MEM_WRITE | HV_MEM_EXEC,
            )
        };
        if ret != HV_SUCCESS {
            unsafe { libc::munmap(mem as *mut libc::c_void, mem_size) };
            anyhow::bail!("hv_vm_map failed: ret={}", ret);
        }

        // Create vCPU
        let mut vcpu: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut exit_info: *const HvVcpuExit = std::ptr::null();
        let ret = unsafe { hv_vcpu_create(&mut vcpu, &mut exit_info, HV_VCPU_DEFAULT) };
        if ret != HV_SUCCESS {
            unsafe { hv_vm_destroy() };
            unsafe { libc::munmap(mem as *mut libc::c_void, mem_size) };
            anyhow::bail!("hv_vcpu_create failed: ret={}", ret);
        }

        init_vcpu_regs(vcpu)?;

        Ok(VmInstance {
            vcpu,
            exit_info,
            vm_id,
            mem,
            mem_size,
        })
    }
}

impl Drop for Hypervisor {
    fn drop(&mut self) {
        unsafe { hv_vm_destroy() };
    }
}

impl Drop for VmInstance {
    fn drop(&mut self) {
        unsafe { hv_vcpu_destroy(self.vcpu) };
        if !self.mem.is_null() {
            unsafe { libc::munmap(self.mem as *mut libc::c_void, self.mem_size) };
        }
    }
}

fn init_vcpu_regs(vcpu: *mut core::ffi::c_void) -> Result<()> {
    let regs: &[(u32, u64)] = &[
        (HV_REG_PC, 0),
        (HV_REG_SP, 0x3FFFF0),
        (HV_REG_CPSR, 0x3C5),
    ];
    for &(reg, val) in regs {
        let ret = unsafe { hv_vcpu_set_reg(vcpu, reg, val) };
        if ret != HV_SUCCESS {
            anyhow::bail!("hv_vcpu_set_reg({}) failed: ret={}", reg, ret);
        }
    }
    Ok(())
}

/// Identity-map first 2GB using level-2 page table at GPA 0x1000.
fn setup_page_tables(mem: *mut u8) {
    let l2 = unsafe { &mut *(mem.add(0x1000) as *mut [u64; 512]) };
    for i in 0..512 {
        l2[i] = (i as u64) << 21 | 0b1000000111;
    }
}

/// Read the hypercall frame from guest memory at HYPERCALL_GPA.
pub fn read_hypercall_frame(vm: &VmInstance) -> HypercallFrame {
    unsafe { *(vm.mem.add(HYPERCALL_GPA as usize) as *const HypercallFrame) }
}

pub fn write_hypercall_status(vm: &VmInstance, status: i32) {
    unsafe {
        *(vm.mem.add(HYPERCALL_GPA as usize) as *mut i32).add(1) = status;
    }
}

#[repr(C, align(64))]
#[derive(Clone, Copy)]
pub struct HypercallFrame {
    pub command_id: u32,
    pub status: i32,
    pub arg0: u64,
    pub arg1: u64,
    pub arg2: u64,
    pub arg3: u64,
}

/// Run the vCPU until a VM exit.
pub fn run_vcpu(vm: &mut VmInstance) -> Result<VmExit> {
    let ret = unsafe { hv_vcpu_run(vm.vcpu) };
    if ret != HV_SUCCESS {
        anyhow::bail!("hv_vcpu_run failed: ret={}", ret);
    }

    let exit = unsafe { &*vm.exit_info };

    match exit.reason {
        HV_EXIT_REASON_HVC => {
            let frame = read_hypercall_frame(vm);
            Ok(VmExit::Hypercall {
                cmd: frame.command_id,
            })
        }
        HV_EXIT_REASON_VM_STOP => Ok(VmExit::Halt),
        HV_EXIT_REASON_CANCEL => Ok(VmExit::Cancelled),
        r => Ok(VmExit::Unknown(r)),
    }
}

pub enum VmExit {
    Hypercall { cmd: u32 },
    Halt,
    Cancelled,
    Unknown(u32),
}