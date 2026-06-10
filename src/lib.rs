//! Moadim server library. Exports the WASM interface when compiled for `wasm32`.

/// WASM bindings for browser-side use.
#[cfg(target_arch = "wasm32")]
pub mod wasm;
