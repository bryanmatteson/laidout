#[path = "support/harness_v1.rs"]
mod harness_v1;

#[path = "adapters/laidout_0_1_baseline.rs"]
#[allow(dead_code)]
mod laidout_0_1_baseline;

#[path = "adapters/prepared_kernel.rs"]
mod prepared_kernel;

fn main() {
    // `cargo test --all-targets` executes harness-less benchmarks without a
    // subcommand. Treat that invocation as the benchmark target's smoke test;
    // explicit capture/compare commands still flow through the frozen harness.
    if std::env::args_os().nth(1).is_none() {
        return;
    }
    let final_role = std::env::args().any(|argument| argument == "prepared-kernel");
    let result = if final_role {
        harness_v1::entry(&prepared_kernel::PreparedAdapter)
    } else {
        harness_v1::entry(&laidout_0_1_baseline::BaselineAdapter)
    };
    if let Err(error) = result {
        eprintln!("prepared-kernel benchmark failed: {error}");
        std::process::exit(1);
    }
}
