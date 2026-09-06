fn main() {
    println!("cargo:rerun-if-env-changed=EMBER_BUILD_VERSION");
    println!("cargo:rerun-if-env-changed=EMBER_BUILD_COMMIT");
}
