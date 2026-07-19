use std::path::Path;

/// Build the React `client/` app and write its self-contained `index.html` to `$OUT_DIR`.
///
/// Runs on every `cargo build`. If `pnpm` is absent the build falls back to a committed prebuilt
/// copy, then a placeholder. Install pnpm from <https://pnpm.io/installation> and run
/// `pnpm install` at the repo root.
///
/// `vite-plugin-singlefile` already makes `client/dist/index.html` fully self-contained — this
/// just needs to copy it, no inlining step.
pub fn build(manifest_dir: &str) {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let output = Path::new(&out_dir).join("index.html");
    // Prebuilt lives at the package root — NOT under `client/` — because `client/` has its own
    // `package.json` but is not a Cargo workspace member, and keeping the fallback at the root
    // package's path keeps it included in `cargo package`/`cargo publish` regardless.
    let prebuilt = Path::new(manifest_dir).join("prebuilt.html");
    let client_dir = Path::new(manifest_dir).join("client");

    if client_dir.exists() {
        emit_rerun_triggers(&client_dir);
        if run_pnpm_build(manifest_dir) {
            let dist_index = client_dir.join("dist").join("index.html");
            if dist_index.exists() {
                std::fs::copy(&dist_index, &output).expect("failed to copy client dist/index.html");
                // Write to the package root so CI can commit it and `cargo install` (no pnpm
                // available) still ships a working UI.
                std::fs::copy(&output, &prebuilt).ok();
                return;
            }
        }
    }

    // pnpm unavailable or build failed — use the prebuilt bundled in the crate.
    if prebuilt.exists() {
        std::fs::copy(&prebuilt, &output).expect("failed to copy prebuilt client UI");
        return;
    }

    println!("cargo:warning=pnpm not found and no prebuilt UI; showing placeholder");
    std::fs::write(&output, PLACEHOLDER_HTML).expect("failed to write placeholder client HTML");
}

/// Emit `cargo:rerun-if-changed` directives for the client's source files.
fn emit_rerun_triggers(client_dir: &Path) {
    println!(
        "cargo:rerun-if-changed={}",
        client_dir.join("src").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        client_dir.join("index.html").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        client_dir.join("vite.config.ts").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        client_dir.join("package.json").display()
    );
}

/// Run `pnpm --filter client build` from `manifest_dir` (the workspace root, so pnpm's workspace
/// resolution finds `client/`). Returns true on success.
fn run_pnpm_build(manifest_dir: &str) -> bool {
    match std::process::Command::new("pnpm")
        .args(["--filter", "client", "build"])
        .current_dir(manifest_dir)
        .status()
    {
        Ok(status) if status.success() => true,
        Ok(_) => {
            println!("cargo:warning=pnpm build exited with non-zero status");
            false
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            println!(
                "cargo:warning=pnpm not found; React client not built \
                 (install from https://pnpm.io/installation, then `pnpm install`)"
            );
            false
        }
        Err(err) => {
            println!("cargo:warning=failed to launch pnpm: {err}");
            false
        }
    }
}

/// Minimal HTML page shown when the React UI has not been built.
const PLACEHOLDER_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><title>MOADIM</title></head>
<body><p>Client not built. Install pnpm (https://pnpm.io/installation), run `pnpm install` at the repo root, and rebuild from source, or reinstall a release that bundles the prebuilt client.</p></body>
</html>"#;
