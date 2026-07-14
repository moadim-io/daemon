---
"moadim": patch
---

fix(build): regenerate stale `prebuilt.html`

`prebuilt.html` last regenerated at #1122 no longer matches the compiled
`ui/` sources — merges since then (e.g. #1129, #1136) drifted the committed
bundle again, so `main` currently fails its own `prebuilt-html-fresh` CI
check and every open PR touching `ui/` inherits that failure regardless of
its own diff. Rebuilt via `cargo check` (trunk 0.21.14, matching the
workflow's pin) and committed the result. No source change.
