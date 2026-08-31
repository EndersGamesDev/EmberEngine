//! Native entry point. Everything lives in the library so the wasm build
//! (`--lib`, a cdylib) and this binary run exactly the same game.

fn main() {
    ember_engine::init_diagnostics();
    fire::run_local();
}
