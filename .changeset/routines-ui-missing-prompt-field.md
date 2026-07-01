---
"moadim": patch
---

### Fixed

- **The routines page failed to load with "missing field `prompt`".** PR #825
  made `GET /routines` omit the `prompt` field from each routine's JSON by
  default, but the UI's separately-mirrored `Routine` struct never got a
  matching `#[serde(default)]` on that field, so the wasm client's
  deserialization broke on every list fetch.
