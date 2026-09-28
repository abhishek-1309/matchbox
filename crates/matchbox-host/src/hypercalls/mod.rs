use anyhow::Result;

#[repr(u32)]
pub enum HypercallCommand {
    Mkdir = 0x01,
    WriteFile = 0x02,
    CopyFile = 0x03,
    HttpFetch = 0x04,
    Exit = 0x05,
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_mkdir() {
        assert!(matches!(HypercallFrame::decode(0x01).unwrap(), HypercallCommand::Mkdir));
    }

    #[test]
    fn decode_write_file() {
        assert!(matches!(HypercallFrame::decode(0x02).unwrap(), HypercallCommand::WriteFile));
    }

    #[test]
    fn decode_copy_file() {
        assert!(matches!(HypercallFrame::decode(0x03).unwrap(), HypercallCommand::CopyFile));
    }

    #[test]
    fn decode_http_fetch() {
        assert!(matches!(HypercallFrame::decode(0x04).unwrap(), HypercallCommand::HttpFetch));
    }

    #[test]
    fn decode_exit() {
        assert!(matches!(HypercallFrame::decode(0x05).unwrap(), HypercallCommand::Exit));
    }

    #[test]
    fn decode_unknown() {
        assert!(HypercallFrame::decode(0xFF).is_err());
    }

    #[test]
    fn frame_layout() {
        let frame = HypercallFrame {
            command_id: 0x01,
            status: 0,
            arg0: 0xAABBCCDD00112233,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        };
        let ptr = &frame as *const _ as *const u8;
        unsafe {
            // command_id at offset 0
            assert_eq!(std::ptr::read_unaligned(ptr as *const u32), 0x01);
            // status at offset 4
            assert_eq!(std::ptr::read_unaligned(ptr.add(4) as *const i32), 0);
            // arg0 at offset 8
            assert_eq!(std::ptr::read_unaligned(ptr.add(8) as *const u64), 0xAABBCCDD00112233);
        }
    }
}