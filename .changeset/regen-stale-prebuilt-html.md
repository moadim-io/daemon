---
"moadim": patch
---

fix(build): regenerate stale `prebuilt.html`

The committed `prebuilt.html` (last regenerated at #1092) no longer matches
the compiled `ui/` sources — every subsequent merge to `ui/src` recompiles
the embedded JS/WASM bytes, so the `prebuilt-html-fresh` CI job now fails on
any PR touching `ui/` even when that PR itself makes no visual change (see
#1119, #1120, #1113). Rebuilding via `cargo check` (which runs `build.rs` /
`trunk`) and committing the result restores a clean baseline so those and
future `ui/` PRs can pass the freshness check again. No source change.
