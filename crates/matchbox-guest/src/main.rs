#![no_std]
#![no_main]

#[cfg(target_arch = "x86_64")]
mod hypercall;

use core::panic::PanicInfo;

#[cfg(target_arch = "x86_64")]
#[unsafe(link_section = ".text.entry")]
#[unsafe(no_mangle)]
pub extern "C" fn _rust_entry() -> ! {
    let _ = hypercall::fs::mkdir("demo");
    hypercall::hypercall(hypercall::Command::Exit);
    loop {
        unsafe { core::arch::asm!("hlt", options(nomem, nostack)); }
    }
}

#[cfg(not(target_arch = "x86_64"))]
#[unsafe(no_mangle)]
pub extern "C" fn _rust_entry() -> ! {
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[global_allocator]
static ALLOC: BumpAllocator = BumpAllocator;

struct BumpAllocator;

unsafe impl core::alloc::GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, _layout: core::alloc::Layout) -> *mut u8 {
        core::ptr::null_mut()
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {}
}
