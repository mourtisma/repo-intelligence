//! Command-line entry point, wiring the analysis engine and detectors crates.

fn main() {
    eprintln!("repo-intelligence {}", env!("CARGO_PKG_VERSION"));
}
