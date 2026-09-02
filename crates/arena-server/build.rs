//! Ties the binary's build stamp to the environment that produced it.
//!
//! `src/lib.rs` reads `EMBER_BUILD_VERSION` and `EMBER_BUILD_COMMIT` through
//! `option_env!`, which resolves at COMPILE time. Without these lines a
//! rebuild from an unchanged source tree is a cache hit, and a host would
//! publish one commit into the address book while its server kept telling
//! every player the previous one — fatal for a book whose whole ranking rule
//! is "newest build wins".

// A build script's only channel to cargo is stdout; printing IS its interface.
#![allow(clippy::print_stdout)]

fn main() {
    println!("cargo:rerun-if-env-changed=EMBER_BUILD_VERSION");
    println!("cargo:rerun-if-env-changed=EMBER_BUILD_COMMIT");
}
