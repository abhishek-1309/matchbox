//! Apple Hypervisor.framework bindings (BLOCKED on ARM64 - hv_vcpu_run hangs).
//! On macOS 15 Apple Silicon, the old API (hv_vcpu_t) hangs in hv_vcpu_run.
//! The new API (hv_vcpuid_t) is x86_64-only.
//! This module compiles but fails at runtime. Development focused on Linux/KVM.

use anyhow::Result;

const GUEST_SIZE: usize = 0x40_0000;
const HYPERCALL_GPA: u64 = 0x300000;

type HvVcpu = u64;

#[link(name = "Hypervisor", kind = "framework")]
extern "C" {
    fn hv_vm_create(options: u64) -> i32;
    fn hv_vm_destroy() -> i32;
    fn hv_vm_map(addr: *const u8, gpa: u64, size: u64, flags: u64) -> i32;
    fn hv_vcpu_create(
        vcpu: *mut HvVcpu,
        exit: *mut *const HvVcpuExit,
        options: u64,
    ) -> i32;
    fn hv_vcpu_destroy(vcpu: HvVcpu) -> i32;
    fn hv_vcpu_run(vcpu: HvVcpu) -> i32;
    fn hv_vcpu_set_reg(vcpu: HvVcpu, reg: u32, val: u64) -> i32;
    fn hv_vcpu_get_reg(vcpu: HvVcpu, reg: u32, val: *mut u64) -> i32;
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
const HV_EXIT_REASON_HVC: u32 = 5;
const HV_EXIT_REASON_VM_STOP: u32 = 9;
const HV_EXIT_REASON_CANCEL: u32 = 0;

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
    pub vcpu: HvVcpu,
    exit_info: *const HvVcpuExit,
    pub vm_id: u32,
    mem: *mut u8,
    mem_size: usize,
}

impl Hypervisor {
    pub fn new() -> Result<Self> {
        let ret = unsafe { hv_vm_create(HV_VM_DEFAULT) };
        if ret != HV_SUCCESS { anyhow::bail!("hv_vm_create: ret={}", ret); }
        Ok(Self)
    }

    pub fn create_vm(&self, vm_id: u32, _mem_size: usize, guest_code: &[u8]) -> Result<VmInstance> {
        let mem_size = GUEST_SIZE;
        let mem = unsafe {
            let ptr = libc::mmap(
                std::ptr::null_mut(), mem_size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS, -1, 0,
            );
            if ptr == libc::MAP_FAILED { anyhow::bail!("mmap: {}", std::io::Error::last_os_error()); }
            ptr as *mut u8
        };
        unsafe { std::ptr::copy_nonoverlapping(guest_code.as_ptr(), mem, guest_code.len()); }
        setup_page_tables(mem);

        let ret = unsafe { hv_vm_map(mem as *const u8, 0, mem_size as u64,
            HV_MEM_READ | HV_MEM_WRITE | HV_MEM_EXEC) };
        if ret != HV_SUCCESS { unsafe { libc::munmap(mem as *mut libc::c_void, mem_size) }; anyhow::bail!("hv_vm_map: ret={}", ret); }

        let mut vcpu: HvVcpu = 0;
        let mut exit_ptr: *const HvVcpuExit = std::ptr::null();
        let ret = unsafe { hv_vcpu_create(&mut vcpu, &mut exit_ptr, HV_VCPU_DEFAULT) };
        if ret != HV_SUCCESS { unsafe { hv_vm_destroy() }; unsafe { libc::munmap(mem as *mut libc::c_void, mem_size) }; anyhow::bail!("hv_vcpu_create: ret={}", ret); }

        eprintln!("[hv] vcpu={} exit_ptr={:p}", vcpu, exit_ptr);

        if !exit_ptr.is_null() {
            let exit = unsafe { &*exit_ptr };
            eprintln!("[hv] initial exit->reason = {}", exit.reason);
        }

        init_vcpu_regs(vcpu)?;
        Ok(VmInstance { vcpu, exit_info: exit_ptr, vm_id, mem, mem_size })
    }
}

impl Drop for Hypervisor {
    fn drop(&mut self) { unsafe { hv_vm_destroy() }; }
}
impl Drop for VmInstance {
    fn drop(&mut self) {
        unsafe { hv_vcpu_destroy(self.vcpu) };
        if !self.mem.is_null() { unsafe { libc::munmap(self.mem as *mut libc::c_void, self.mem_size) }; }
    }
}

fn init_vcpu_regs(vcpu: HvVcpu) -> Result<()> {
    for &(reg, val) in &[(HV_REG_PC, 0u64), (HV_REG_SP, 0x3FFFF0), (HV_REG_CPSR, 0x3C5)] {
        let ret = unsafe { hv_vcpu_set_reg(vcpu, reg, val) };
        if ret != HV_SUCCESS { anyhow::bail!("hv_vcpu_set_reg({}): ret={}", reg, ret); }
    }
    Ok(())
}

fn setup_page_tables(mem: *mut u8) {
    let l2 = unsafe { &mut *(mem.add(0x1000) as *mut [u64; 512]) };
    for i in 0..512 { l2[i] = (i as u64) << 21 | 0b1000000111; }
}

pub fn read_hypercall_frame(vm: &VmInstance) -> HypercallFrame {
    unsafe { *(vm.mem.add(HYPERCALL_GPA as usize) as *const HypercallFrame) }
}
pub fn write_hypercall_status(vm: &VmInstance, status: i32) {
    unsafe { *(vm.mem.add(HYPERCALL_GPA as usize) as *mut i32).add(1) = status; }
}

#[repr(C, align(64))]
#[derive(Clone, Copy)]
pub struct HypercallFrame {
    pub command_id: u32, pub status: i32, pub arg0: u64, pub arg1: u64, pub arg2: u64, pub arg3: u64,
}

pub fn run_vcpu(vm: &mut VmInstance) -> Result<VmExit> {
    eprintln!("[hv] calling hv_vcpu_run...");
    let ret = unsafe { hv_vcpu_run(vm.vcpu) };
    eprintln!("[hv] hv_vcpu_run returned: ret={}", ret);

    if ret != HV_SUCCESS { anyhow::bail!("hv_vcpu_run: ret={}", ret); }

    if vm.exit_info.is_null() {
        eprintln!("[hv] exit_info is NULL!");
        return Ok(VmExit::Unknown(0));
    }

    let exit = unsafe { &*vm.exit_info };
    eprintln!("[hv] exit reason = {}", exit.reason);

    match exit.reason {
        HV_EXIT_REASON_HVC => {
            let frame = read_hypercall_frame(vm);
            Ok(VmExit::Hypercall { cmd: frame.command_id })
        }
        HV_EXIT_REASON_VM_STOP => Ok(VmExit::Halt),
        HV_EXIT_REASON_CANCEL => Ok(VmExit::Cancelled),
        r => Ok(VmExit::Unknown(r)),
    }
}

pub enum VmExit {
    Hypercall { cmd: u32 }, Halt, Cancelled, Unknown(u32),
}