#[cfg(not(target_arch = "wasm32"))]
mod server;

mod wasm;

#[cfg(target_arch = "wasm32")]
fn main() {
    wasm::wasm_start();
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> std::io::Result<()> {
    server::run().await
}
