// ARM64 boot stub for Hypervisor.framework (EL1 entry).
//
// Memory layout:
//   0x000: _start (this code)
//   0x100: _rust_entry (Rust entry point)
//   0x1000: Level-1 page table (2MB block entries, identity map)
//   0x2000_0000: Hypercall frame

.section .text.boot
.global _start
.type _start, @function

_start:
    // Set up page table address
    ldr x1, =0x1000
    msr ttbr0_el1, x1

    // Configure TCR_EL1
    // TG0=4KB (bits 15-14=00), SH0=IS (bits 13-12=11),
    // ORGN0=WB (bits 11-10=11), IRGN0=WB (bits 9-8=11),
    // T0SZ=24 (bits 5-0=11000, 48-bit VA), IPS=40bit (bits 34-32=101)
    ldr x2, =0x80853510
    msr tcr_el1, x2

    // Configure MAIR_EL1
    // Attr0=0xFF (Normal WB WA RA), Attr1=0x44 (Device nGnRE)
    ldr x3, =0x44ff044
    msr mair_el1, x3

    // Set stack pointer near top of 4MB
    ldr x4, =0x3FFFF0
    mov sp, x4

    // Enable MMU
    mrs x5, sctlr_el1
    orr x5, x5, #0x1        // M bit (MMU enable)
    orr x5, x5, #0x1000     // I bit (icache)
    orr x5, x5, #0x4        // C bit (dcache)
    msr sctlr_el1, x5
    isb

    // Jump to Rust entry at 0x100
    ldr x6, =0x100
    br x6

.balign 0x100
.global _rust_entry
.type _rust_entry, @function
_rust_entry:
    // Simply hang with WFI for now
    wfi
    b _rust_entry