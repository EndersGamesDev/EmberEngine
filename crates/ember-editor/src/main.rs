//! Native shell for the editor.
//!
//! The native build exists before the web one on purpose: it needs no wasm
//! plumbing, no deploy change and no engine change, so it is a window a peer
//! with a toolchain can open on day one. The web shell (milestone 2 bite 9)
//! adds the DOM sidebar the user asked for; both drive the same editor
//! through one command queue, so neither is a throwaway.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    ember_engine::init_diagnostics();
    ember_engine::run(ember_editor::engine_config(), ember_editor::Editor::new());
}

/// The web shell enters through `start_editor` in the library, not through
/// a binary. This exists only so `cargo check --target wasm32` covers the
/// whole crate rather than stopping at a bin that cannot build there.
#[cfg(target_arch = "wasm32")]
fn main() {}
