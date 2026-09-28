#![no_std]
#![no_main]
#![cfg_attr(target_arch = "x86_64", feature(asm_experimental_arch))]

#[cfg(target_arch = "x86_64")]
mod hypercall;

use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "C" fn _rust_entry() -> ! {
    loop {
        #[cfg(target_arch = "x86_64")]
        unsafe { core::arch::asm!("hlt"); }
    }
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
