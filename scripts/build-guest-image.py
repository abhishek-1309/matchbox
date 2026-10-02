#!/usr/bin/env python3
"""Build matchbox-guest and write the raw image to crates/matchbox-guest/guest.bin.

The image is linked at GPA 0x8000. Byte 0 of the file is the byte at that address.
"""

import os
import struct
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LOAD = 0x8000
PT_LOAD = 1
OUT = ROOT / "crates" / "matchbox-guest" / "guest.bin"


def elf_image(blob: bytes) -> bytes:
    if blob[:4] != b"\x7fELF" or blob[4] != 2 or blob[5] != 1:
        raise SystemExit("guest build did not produce a 64-bit little-endian ELF")
    e_phoff = struct.unpack_from("<Q", blob, 32)[0]
    e_phentsize, e_phnum = struct.unpack_from("<HH", blob, 54)
    end = LOAD
    loads = []
    for i in range(e_phnum):
        off = e_phoff + i * e_phentsize
        p_type, _, p_offset, p_vaddr, _, p_filesz, p_memsz, _ = struct.unpack_from(
            "<IIQQQQQQ", blob, off
        )
        if p_type != PT_LOAD or p_memsz == 0:
            continue
        if p_vaddr < LOAD:
            raise SystemExit(f"PT_LOAD at {p_vaddr:#x} is below {LOAD:#x}")
        loads.append((p_vaddr, p_offset, p_filesz, p_memsz))
        end = max(end, p_vaddr + p_memsz)
    if not loads:
        raise SystemExit("guest ELF has no PT_LOAD segments")
    image = bytearray(end - LOAD)
    for p_vaddr, p_offset, p_filesz, _p_memsz in loads:
        start = p_vaddr - LOAD
        chunk = blob[p_offset : p_offset + p_filesz]
        image[start : start + len(chunk)] = chunk
    return bytes(image)


def main() -> None:
    subprocess.run(
        [
            "cargo",
            "build",
            "-p",
            "matchbox-guest",
            "--target",
            "x86_64-unknown-none",
        ],
        cwd=ROOT,
        check=True,
    )
    target_dir = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    elf = (
        target_dir
        / "x86_64-unknown-none"
        / "debug"
        / "matchbox-guest"
    ).read_bytes()
    image = elf_image(elf)
    if b"demo" not in image or b"\xef" not in image:
        raise SystemExit("guest image is missing the entry hypercall")
    OUT.write_bytes(image)
    print(f"wrote {OUT} ({len(image)} bytes)")


if __name__ == "__main__":
    sys.exit(main())
