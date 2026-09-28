use core::arch::asm;

const HYPERCALL_FRAME_ADDR: u64 = 0x2000_0000;
const HYPERCALL_PORT: u16 = 0x3F0;

#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Command {
    Mkdir = 0x01,
    WriteFile = 0x02,
    CopyFile = 0x03,
    HttpFetch = 0x04,
    Exit = 0x05,
}

#[repr(C, align(64))]
pub struct Frame {
    pub command_id: u32,
    pub status: i32,
    pub arg0: u64,
    pub arg1: u64,
    pub arg2: u64,
    pub arg3: u64,
}

impl Frame {
    pub fn at_default() -> &'static mut Self {
        unsafe { &mut *(HYPERCALL_FRAME_ADDR as *mut Self) }
    }
}

#[inline(always)]
pub fn hypercall(cmd: Command) {
    let frame = Frame::at_default();
    frame.command_id = cmd as u32;
    frame.status = 0;

    unsafe {
        asm!(
            "outl %eax, %dx",
            in("eax") cmd as u32,
            in("dx") HYPERCALL_PORT,
            options(nomem, nostack),
        );
    }
}

pub mod fs {
    use super::*;

    pub fn mkdir(path: &str) -> i32 {
        let frame = Frame::at_default();
        frame.arg0 = path.as_ptr() as u64;
        frame.arg1 = path.len() as u64;
        hypercall(Command::Mkdir);
        frame.status
    }

    pub fn write_file(path: &str, data: &[u8]) -> i32 {
        let frame = Frame::at_default();
        frame.arg0 = path.as_ptr() as u64;
        frame.arg1 = path.len() as u64;
        frame.arg2 = data.as_ptr() as u64;
        frame.arg3 = data.len() as u64;
        hypercall(Command::WriteFile);
        frame.status
    }
}

pub mod net {
    use super::*;

    pub fn fetch(url: &str, out_buf: &mut [u8]) -> i32 {
        let frame = Frame::at_default();
        frame.arg0 = url.as_ptr() as u64;
        frame.arg1 = url.len() as u64;
        frame.arg2 = out_buf.as_ptr() as u64;
        frame.arg3 = out_buf.len() as u64;
        hypercall(Command::HttpFetch);
        frame.status
    }
}
