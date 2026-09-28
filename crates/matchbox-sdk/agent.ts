// Agent SDK for AI agents running inside the MicroVM.
// These wrappers will be exposed to MicroPython scripts.

export class Fs {
  static mkdir(path: string): number {
    // Maps to CMD_MKDIR hypercall
    // Implemented via FFI to guest hypercall::fs::mkdir
    return 0;
  }

  static writeFile(path: string, data: Uint8Array): number {
    // Maps to CMD_WRITE_FILE hypercall
    return 0;
  }

  static copyFile(src: string, dst: string): number {
    // Maps to CMD_COPY_FILE hypercall
    return 0;
  }
}

export class Net {
  static fetch(url: string, outBuf: Uint8Array): number {
    // Maps to CMD_HTTP_FETCH hypercall
    return 0;
  }
}