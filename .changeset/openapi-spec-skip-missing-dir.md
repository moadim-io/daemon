---
"moadim": patch
---

fix(server): stop warning on every startup about the openapi.json write

`write_openapi_spec` targets `CARGO_MANIFEST_DIR/apis/openapi.json`, a path
baked in at compile time. For an installed binary (`cargo install`), that
directory is wherever the crate happened to build and generally doesn't
exist on the end user's machine, so every server startup logged a
`could not write openapi spec: ...` warning for a file nobody expects to be
writable there (#319). Skip the write when its parent directory doesn't
exist instead of attempting and warning.
