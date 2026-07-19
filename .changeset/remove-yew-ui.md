---
"moadim": minor
---

feat(ui): remove the legacy Yew UI (`ui/`), make the React client (`client/`) the sole embedded UI

The daemon has shipped two parallel browser UIs since the React client's rollout began: the
original Yew/WASM SPA at `/` and the React client at `/client`. The Yew crate is now removed and
the React client takes over `/` as the one and only UI — no more dual-maintenance, dual-build, or
dual-CSP-allowance burden.

- Deleted the `ui/` Cargo workspace member (Yew/WASM crate) entirely, along with its Trunk build
  step (`src/build/ui.rs`), workspace membership (`Cargo.toml`), and CI jobs (`clippy-ui-wasm`,
  `prebuilt-ui.yml`).
- `src/build/client.rs` (the former `/client` builder) is now the only UI builder — it writes
  `$OUT_DIR/index.html` / `prebuilt.html` directly, replacing the old Yew-inlining build step.
- `GET /` now serves the React client; the `/client` route and its nested router are gone.
- `prebuilt-client.html` renamed to `prebuilt.html` (the old Yew `prebuilt.html` is deleted); its
  freshness-check workflow renamed `prebuilt-ui.yml` → `prebuilt.yml`.
- Dropped `'wasm-unsafe-eval'` from the CSP's `script-src` — no WASM SPA is served anymore, so the
  narrower policy is strictly tighter than before.
- Updated `Architecture.md`, `CONTRIBUTING.md`, `.githooks/pre-push`, and the remaining CI
  workflows (`lint.yml`, `test.yml`, `publish.yml`, `changelog.yml`) to drop every `ui/` reference.

No REST/MCP API changes. The `/ui` back-compat redirect to `/` is unaffected.
