fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();

    // Custom linker script for bare-metal targets
    if target == "x86_64-unknown-none" || target == "x86_64-unknown-elf" {
        println!("cargo:rustc-link-arg=-T{}/linker.ld",
            std::env::var("CARGO_MANIFEST_DIR").unwrap());
    }
}