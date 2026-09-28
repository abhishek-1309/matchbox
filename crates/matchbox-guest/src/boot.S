# Global Descriptor Table
.section .rodata
.align 8
gdt:
    .quad 0x0000000000000000  # Null descriptor
    .quad 0x00CF9A000000FFFF  # Ring 0 code (64-bit)
    .quad 0x00CF92000000FFFF  # Ring 0 data

gdt_ptr:
    .word (gdt_ptr - gdt - 1)
    .quad gdt

# Page table for identity mapping first 4 MB
.section .bss
.align 4096
pml4:
    .space 4096
pdpt:
    .space 4096
pd:
    .space 4096
pt:
    .space 4096

.section .text.boot
.code16
.global _start
_start:
    cli
    mov %ax, %cx
    # Enable A20 gate
    in $0x92, %al
    or $2, %al
    out %al, $0x92

    # Load GDT
    lgdt gdt_ptr

    # Enable protected mode
    mov %cr0, %eax
    or $1, %eax
    mov %eax, %cr0

    # Jump to 32-bit protected mode
    ljmp $0x08, $protected_mode

.code32
protected_mode:
    # Set up data segments
    mov $0x10, %ax
    mov %ax, %ds
    mov %ax, %es
    mov %ax, %fs
    mov %ax, %gs
    mov %ax, %ss

    # Set up page tables (identity map)
    # PML4 entry 0 -> PDPT
    mov $pdpt, %eax
    or $0x03, %eax
    mov %eax, pml4

    # PDPT entry 0 -> PD
    mov $pd, %eax
    or $0x03, %eax
    mov %eax, pdpt

    # PD entry 0 -> PT (2 MB page)
    mov $pt, %eax
    or $0x03, %eax
    mov %eax, pd

    # PT entries: identity map first 4 MB
    mov $0, %ebx
    mov $512, %ecx
    mov $pt, %edi
1:
    mov %ebx, %eax
    or $0x03, %eax
    mov %eax, (%edi)
    add $4096, %ebx
    add $8, %edi
    dec %ecx
    jnz 1b

    # Load PML4 into CR3
    mov $pml4, %eax
    mov %eax, %cr3

    # Enable PAE
    mov %cr4, %eax
    or $0x20, %eax
    mov %eax, %cr4

    # Set EFER LME for long mode
    mov $0xC0000080, %ecx
    rdmsr
    or $0x100, %eax
    wrmsr

    # Enable paging
    mov %cr0, %eax
    or $0x80000001, %eax
    mov %eax, %cr0

    # Jump to 64-bit long mode
    ljmp $0x08, $long_mode

.code64
long_mode:
    # Set stack pointer
    mov $0x200000, %rsp

    # Jump to Rust entry
    jmp _rust_entry