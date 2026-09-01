//! Command-line entry point for the sole game host.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    ember_server::Host::from_environment()?.run()
}
