/// Guest physical address where the unikernel image is copied.
/// Must match the origin in `matchbox-guest/linker.ld`.
pub const GUEST_LOAD_GPA: u64 = 0x8000;

/// Stack pointer set before entering `_rust_entry`.
pub const GUEST_STACK_GPA: u64 = 0x200000;

pub const GUEST_MEM_SIZE: usize = 4 * 1024 * 1024;

pub fn guest_image() -> &'static [u8] {
    include_bytes!("../../matchbox-guest/guest.bin")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_fits_below_the_stack() {
        let image = guest_image();
        assert!(!image.is_empty());
        let end = GUEST_LOAD_GPA as usize + image.len();
        assert!(end <= GUEST_STACK_GPA as usize);
        assert!(GUEST_STACK_GPA as usize + 4096 <= GUEST_MEM_SIZE);
    }

    #[test]
    fn image_contains_hypercall_out() {
        let image = guest_image();
        assert!(
            image.contains(&0xEF),
            "guest image is missing the OUT instruction used by hypercalls"
        );
        assert!(
            image.windows(4).any(|w| w == b"demo"),
            "guest image is missing the demo path the entry point creates"
        );
    }
}
