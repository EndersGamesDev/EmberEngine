//! Re-run the build when the deploy's stamp changes, so a binary built after
//! `EMBER_BUILD_VERSION` / `EMBER_BUILD_COMMIT` moved carries the new stamp
//! rather than the cached one. The values themselves are read in `lib.rs`
//! through `option_env!`; a plain `cargo build` produces an unstamped binary
//! that says so in its first log line (`docs/hosts.md`, section 4).

// A build script speaks to cargo on stdout; that is the protocol.
#![allow(clippy::print_stdout)]

fn main() {
    println!("cargo:rerun-if-env-changed=EMBER_BUILD_VERSION");
    println!("cargo:rerun-if-env-changed=EMBER_BUILD_COMMIT");
}
