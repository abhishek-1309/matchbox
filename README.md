# Matchbox

**M:N Fiber-Scheduled MicroVM Engine** — Lightweight, capability-isolated micro-VMs for AI agent workloads.

```
                    ┌───────────────────────────────┐
                    │        AI AGENT WORKFLOW       │
                    │  (10,000 isolated tool calls)  │
                    └──────────────┬────────────────┘
                                  │ Unix Socket / IPC
                                  ▼
┌────────────────────────────────────────────────────────────────────┐
│  HOST PLANE (Rust)                                                 │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────────────────┐ │
│  │ M:N Fiber   │  │  Capability  │  │  Async I/O Subsystem     │ │
│  │ Scheduler   │  │  Gatekeeper  │  │  (io_uring / tokio)      │ │
│  │ (crossbeam) │  │ (hypercalls) │  │                          │ │
│  └──────┬──────┘  └──────┬───────┘  └──────────┬───────────────┘ │
│         │                │                     │                  │
│         │     ┌──────────┴──────────┐          │                  │
│         │     │   VM Backend       │          │                  │
│         │     │  Linux: /dev/kvm   │          │                  │
│         │     │  macOS: Hypervisor │          │                  │
│         │     │  .framework        │          │                  │
│         │     └──────────┬──────────┘          │                  │
│         ▼                ▼                     ▼                  │
│  ┌───────────────────────────────────────────────────────────┐    │
│  │           Memory: CoW via memfd / MAP_PRIVATE             │    │
│  │           Each VM starts at < 500 KB dirty RAM            │    │
│  └───────────────────────────────────────────────────────────┘    │
└─────────────────────────────────┬──────────────────────────────────┘
                                  │ Hypercall Trap (outl 0x3F0 / hvc #0)
                                  ▼
┌────────────────────────────────────────────────────────────────────┐
│  GUEST PLANE (#![no_std] Unikernel)                               │
│  ┌────────────────────────────────────────────────────────────┐   │
│  │  Shared Memory (Golden Master + CoW overlay)               │   │
│  │  • boot.S → long mode (x86) / boot_aarch64.S → EL1 (ARM)  │   │
│  │  • Embedded QuickJS / MicroPython                         │   │
│  │  • Hypercall: outl 0x3F0 (x86) / hvc #0 (ARM64)           │   │
│  │  • Bump allocator (no libc)                               │   │
│  └────────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────────┘
```

---

## Backends

| Platform | API | Guest Arch | Status |
|----------|-----|-----------|--------|
| Linux | KVM (`/dev/kvm`) | x86-64 | Compiles, needs Linux host to test |
| macOS | Hypervisor.framework | ARM64 | Working: VM creation, memory mapping, vCPU run, HVC trap |

---

## Getting Started

### macOS

```bash
brew install ldid
git clone https://github.com/your-org/matchbox
cd matchbox

# Build
MACOSX_DEPLOYMENT_TARGET=14.0 cargo build -p matchbox-host

# Sign with Hypervisor entitlement
ldid -M -Shv.entitlements target/debug/matchbox-host

# Run
target/debug/matchbox-host
```

### Linux

```bash
# Requires KVM support
sudo apt install qemu-kvm
ls /dev/kvm  # must exist

cargo run -p matchbox-host
```

---

## Architecture

### Hypercall Protocol

Guest ↔ Host communication is via a shared memory frame at a fixed GPA, triggered by a hardware trap:

```
Guest fills HypercallFrame at GPA 0x300000
  ├── command_id (u32): 0x01 MKDIR, 0x02 WRITE_FILE, ...
  ├── status (i32): set by host after handling
  ├── arg0-arg3: path pointers, data buffers
  └── aligned to 64 bytes

Guest executes: outl $0x3F0, $cmd   (x86)
              or: hvc #0            (ARM64)
                   │
                   ▼
Host reads frame, validates via Capability Gatekeeper,
executes I/O asynchronously, writes status back.
```

### M:N Scheduler

```
                    Global Injector (crossbeam)
                   /         |           \
          Worker 1        Worker 2      Worker N
         /   |   \        /   \           |
      VM A  VM B VM C   VM D  VM E      VM F
```

Each worker thread runs a vCPU via `KVM_RUN` / `hv_vcpu_run`. On VM-Exit (hypercall), it handles the request, marks the fiber as `BlockedOnIo`, and immediately steals the next runnable fiber — no thread blocking.

### Memory Model

- **Golden Master**: one `memfd` containing the guest OS binary, shared read-only
- **Instance**: `mmap(MAP_PRIVATE)` of the golden master — CoW means only dirty pages allocate physical RAM
- **Teardown**: just drop the `mmap` — reclaims memory in microseconds

---

## Project Structure

```
matchbox/
├── Cargo.toml
├── crates/
│   ├── matchbox-host/        # Hypervisor daemon
│   │   └── src/
│   │       ├── main.rs       # Platform dispatch
│   │       ├── kvm/          # Linux KVM backend
│   │       ├── hv/           # macOS Hypervisor.framework backend
│   │       ├── scheduler/    # M:N work-stealing
│   │       ├── memory/       # CoW / memfd
│   │       ├── hypercalls/   # Hypercall handlers
│   │       └── security/     # Path sandboxing
│   ├── matchbox-guest/       # #![no_std] unikernel
│   │   └── src/
│   │       ├── main.rs       # Entry + allocator
│   │       ├── boot.S        # x86-64 boot stub
│   │       ├── boot_aarch64.S # ARM64 boot stub
│   │       ├── hypercall.rs  # x86 outl trampoline
│   │       └── hypercall_aarch64.rs # ARM64 hvc trampoline
│   └── matchbox-sdk/         # Agent SDK (TypeScript)
│       └── agent.ts          # Fs / Net wrappers
├── tests/
│   └── density_bench.rs      # 10k VM spawn benchmark
└── hv.entitlements           # macOS entitlement plist
```

---

## Performance Targets

| Metric | Target | Approach |
|--------|--------|----------|
| Cold-start | < 150 μs | No kernel decompression, no systemd |
| Idle RAM | < 500 KB per VM | CoW via memfd MAP_PRIVATE |
| Density | 10,000 VMs on 32 GB | M:N fibers avoid pthread overhead |
| I/O latency | < 1.5 μs per call | Direct port trapping, no POSIX translation |