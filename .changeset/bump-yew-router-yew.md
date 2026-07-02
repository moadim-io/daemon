---
"moadim": patch
---

### Changed

- Bumped `yew-router` from `0.18` to `0.20` and `yew` from `0.21` to `0.23`
  (a required companion bump — `yew-router` 0.20 depends on `yew` 0.23) in
  the `ui` crate. No source changes needed beyond the version bump; the
  bundled Yew/WASM SPA builds and behaves the same.
