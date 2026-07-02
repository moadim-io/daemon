---
"moadim": patch
---

### Fixed

- **OpenAPI spec version now tracks the crate version.** The generated OpenAPI
  document previously reported a frozen `0.1.0`, regardless of the actual
  `moadim` release. It now derives its `info.version` from `CARGO_PKG_VERSION`
  at build time. (#309)
