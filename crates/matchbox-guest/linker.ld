OUTPUT_ARCH(i386:x86-64)
ENTRY(_rust_entry)

SECTIONS {
    /* Host copies this image to GPA 0x8000 and sets RIP there.
       Page tables occupy 0x1000..0x4000, so the image must stay above them. */
    . = 0x8000;

    .text : {
        *(.text.entry)
        *(.text.boot)
        *(.text .text.*)
    }

    .rodata : {
        *(.rodata .rodata.*)
    }

    .data : {
        *(.data .data.*)
    }

    .bss : {
        *(.bss .bss.*)
        *(COMMON)
    }

    /DISCARD/ : {
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.dynamic)
        *(.dynsym)
        *(.dynstr)
        *(.gnu.hash)
        *(.hash)
        *(.got)
        *(.got.plt)
        *(.rela.*)
        *(.note.*)
        *(.comment)
    }
}