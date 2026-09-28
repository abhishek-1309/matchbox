use anyhow::Result;

/// Hypercall command IDs communicated via I/O port 0x3F0.
#[repr(u32)]
pub enum HypercallCommand {
    Mkdir = 0x01,
    WriteFile = 0x02,
    CopyFile = 0x03,
    HttpFetch = 0x04,
    Exit = 0x05,
}

/// Memory layout of the hypercall request frame at guest physical address 0x2000_0000.
#[repr(C, align(64))]
pub struct HypercallFrame {
    pub command_id: u32,
    pub status: i32,
    pub arg0: u64,
    pub arg1: u64,
    pub arg2: u64,
    pub arg3: u64,
}

impl HypercallFrame {
    pub fn decode(data: u32) -> Result<HypercallCommand> {
        match data {
            0x01 => Ok(HypercallCommand::Mkdir),
            0x02 => Ok(HypercallCommand::WriteFile),
            0x03 => Ok(HypercallCommand::CopyFile),
            0x04 => Ok(HypercallCommand::HttpFetch),
            0x05 => Ok(HypercallCommand::Exit),
            _ => anyhow::bail!("unknown hypercall command: {:#x}", data),
        }
    }
}