---
"moadim": patch
---

fix(build): inline the compiled CSS into `prebuilt.html`, not just JS/WASM

`src/build/ui.rs`'s `inline_into_html` folded trunk's compiled JS and WASM
into a single self-contained `index.html`, but left the CSS as an external
`<link rel="stylesheet" href="./styles-<hash>.css">` — a file that never
gets embedded or shipped alongside `prebuilt.html` (only the HTML itself is
copied to the package root and committed). Every `cargo install moadim`
user hit this: the server's catch-all route serves `index.html` for that
missing CSS request, the browser gets `text/html` back for a `.css`
request, and strict MIME checking refuses to apply it — the control panel
rendered completely unstyled. Present since CSS inlining was never added
alongside JS/WASM inlining (reproduces on the v0.26.0 tag too); this is the
first release to carry the fix.

`find_dist_assets` now also locates the `.css` file in trunk's `dist/`,
and `assemble_html` inlines it into a `<style>` block in `<head>` the same
way the WASM bytes are inlined into the boot script, so the served bundle
makes zero external asset requests.
