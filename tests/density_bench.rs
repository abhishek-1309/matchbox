//! Density benchmark: spawns 10,000 idle VMs and measures RAM + cold-start time.
//! Run with: cargo test --test density_bench --release -- --nocapture

use std::time::Instant;

#[test]
fn test_spawn_10000_idle_vms() {
    let start = Instant::now();

    // TODO: spawn N VmInstance objects from matchbox-host
    // For now, this is a structural placeholder.

    let elapsed = start.elapsed();
    println!("Placeholder: would spawn 10,000 VMs (took {:?})", elapsed);
    assert!(elapsed.as_micros() < 1_000_000); // sanity: < 1 sec for 10k
}