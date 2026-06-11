use std::path::{Path, PathBuf};

/// Build the Yew UI and write a self-contained `index.html` to `$OUT_DIR`.
///
/// Runs on every `cargo build`. If trunk is absent the build falls back to
/// a placeholder. Install trunk with: `cargo install trunk`
pub fn build(manifest_dir: &str) {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let output = Path::new(&out_dir).join("index.html");
    // Prebuilt lives at the package root — NOT under `ui/`. `ui/` is a separate
    // workspace member, so `cargo publish` strips everything beneath it; a
    // prebuilt stored there never ships in the published crate. The root path
    // is part of the `moadim` package and is included in the tarball.
    let prebuilt = Path::new(manifest_dir).join("prebuilt.html");
    let ui_dir = Path::new(manifest_dir).join("ui");

    if ui_dir.exists() {
        emit_rerun_triggers(&ui_dir);
        if run_trunk(&ui_dir) {
            let dist = ui_dir.join("dist");
            if dist.exists() {
                inline_into_html(&dist, &output);
                // Write to the package root so CI can commit it and
                // `cargo publish` ships it for the `cargo install` path.
                std::fs::copy(&output, &prebuilt).ok();
                return;
            }
        }
    }

    // Trunk unavailable — use prebuilt bundled in the crate (cargo install path).
    if prebuilt.exists() {
        std::fs::copy(&prebuilt, &output).expect("failed to copy prebuilt UI");
        return;
    }

    println!("cargo:warning=trunk not found and no prebuilt UI; showing placeholder");
    std::fs::write(&output, PLACEHOLDER_HTML).expect("failed to write placeholder HTML");
}

fn emit_rerun_triggers(ui_dir: &Path) {
    println!("cargo:rerun-if-changed={}", ui_dir.join("src").display());
    println!(
        "cargo:rerun-if-changed={}",
        ui_dir.join("index.html").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        ui_dir.join("Trunk.toml").display()
    );
}

/// Run `trunk build --release` in `ui_dir`. Returns true on success.
fn run_trunk(ui_dir: &Path) -> bool {
    match std::process::Command::new("trunk")
        .args(["build", "--release"])
        .current_dir(ui_dir)
        .status()
    {
        Ok(s) if s.success() => true,
        Ok(_) => {
            println!("cargo:warning=trunk build exited with non-zero status");
            false
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!(
                "cargo:warning=trunk not found; Yew UI not compiled \
                 (install with: cargo install trunk)"
            );
            false
        }
        Err(e) => {
            println!("cargo:warning=failed to launch trunk: {e}");
            false
        }
    }
}

/// Read trunk's `dist/` output and produce a single self-contained HTML file.
///
/// Strategy: monkey-patch `fetch` before the wasm-bindgen init call so that
/// any request for a `.wasm` URL returns our base64-encoded bytes instead.
/// This avoids touching wasm-bindgen internals while keeping a single file.
fn inline_into_html(dist: &Path, output: &Path) {
    let html_src = std::fs::read_to_string(dist.join("index.html"))
        .expect("dist/index.html missing after trunk build");

    let (js_path, wasm_path) = find_dist_assets(dist);

    let js = js_path
        .as_ref()
        .map(|p| std::fs::read_to_string(p).expect("failed to read .js dist asset"))
        .unwrap_or_default();

    let wasm_b64 = wasm_path
        .as_ref()
        .map(|p| {
            let bytes = std::fs::read(p).expect("failed to read .wasm dist asset");
            base64_encode(&bytes)
        })
        .unwrap_or_default();

    let wasm_file = wasm_path
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let js_file = js_path
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    // Build the inline <script type="module"> block:
    //   1. Decode WASM from base64 into a Uint8Array.
    //   2. Patch globalThis.fetch so any *.wasm request returns our bytes.
    //   3. Inline the wasm-bindgen JS module (exports are ignored; init() is called).
    //   4. Call await init().
    let inline_script = format!(
        r#"<script type="module">
const __wasm=Uint8Array.from(atob('{wasm_b64}'),c=>c.charCodeAt(0));
const __fetch=globalThis.fetch;
globalThis.fetch=(u,...a)=>String(u).endsWith('.wasm')
  ?Promise.resolve(new Response(__wasm,{{headers:{{'Content-Type':'application/wasm'}}}}))
  :__fetch(u,...a);
{js}
await __wbg_init();
</script>"#
    );

    let final_html = assemble_html(&html_src, &inline_script, &js_file, &wasm_file);

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).expect("failed to create output dir");
    }
    std::fs::write(output, final_html).expect("failed to write inlined index.html");
}

fn find_dist_assets(dist: &Path) -> (Option<PathBuf>, Option<PathBuf>) {
    let mut js_path = None;
    let mut wasm_path = None;
    for entry in std::fs::read_dir(dist).expect("dist dir missing").flatten() {
        let p = entry.path();
        let name = p.file_name().unwrap_or_default().to_string_lossy();
        if name.ends_with(".js") && name != "index.html" {
            js_path = Some(p);
        } else if name.ends_with(".wasm") {
            wasm_path = Some(p);
        }
    }
    (js_path, wasm_path)
}

/// Strip external asset references and inject the inline script.
fn assemble_html(html: &str, inline_script: &str, js_file: &str, wasm_file: &str) -> String {
    // Drop <link> preload/modulepreload lines referencing the generated assets
    let stripped: String = html
        .lines()
        .filter(|line| {
            let l = line.trim();
            !(l.contains(js_file) || l.contains(wasm_file))
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Replace the trunk-generated <script type="module">…</script> with our inline version.
    // Trunk emits exactly one such block; if the pattern changes between trunk versions
    // this falls back to appending before </body>.
    let marker = r#"<script type="module">"#;
    if let Some(start) = stripped.find(marker) {
        if let Some(rel_end) = stripped[start..].find("</script>") {
            let end = start + rel_end + "</script>".len();
            let mut out = stripped.clone();
            out.replace_range(start..end, inline_script);
            return out;
        }
    }

    // Fallback: append before </body>
    stripped.replace("</body>", &format!("{inline_script}\n</body>"))
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let n = match chunk.len() {
            3 => (chunk[0] as usize) << 16 | (chunk[1] as usize) << 8 | chunk[2] as usize,
            2 => (chunk[0] as usize) << 16 | (chunk[1] as usize) << 8,
            1 => (chunk[0] as usize) << 16,
            _ => unreachable!(),
        };
        out.push(TABLE[(n >> 18) & 63] as char);
        out.push(TABLE[(n >> 12) & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n & 63] as char
        } else {
            '='
        });
    }
    out
}

const PLACEHOLDER_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><title>MOADIM</title></head>
<body><p>UI not built. Install trunk (`cargo install trunk`) and rebuild from source, or reinstall a release that bundles the prebuilt UI.</p></body>
</html>"#;
