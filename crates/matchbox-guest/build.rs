fn main() {
    #[cfg(target_arch = "x86_64")]
    {
        // Assemble the boot stub
        cc::Build::new()
            .file("src/boot.S")
            .target("x86_64-unknown-none")
            .compile("boot");
    }

    // Pass the linker script to rustc
    #[cfg(target_arch = "x86_64")]
    println!("cargo:rustc-link-arg=-T{}/linker.ld",
        std::env::var("CARGO_MANIFEST_DIR").unwrap());
}