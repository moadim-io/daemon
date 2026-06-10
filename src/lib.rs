// Native-only server module
#[cfg(not(target_arch = "wasm32"))]
pub mod server;

// WASM-only utilities
#[cfg(target_arch = "wasm32")]
pub mod wasm;
