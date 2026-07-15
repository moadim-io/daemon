---
"moadim": patch
---

chore(lint): enable `clippy::string_lit_as_bytes` in the root crate. Rewrites the two
`"...".as_bytes()` comparisons in `src/routes/http_settings_routes_tests.rs` to byte-string
literal slices (`&b"..."[..]`), stating "this is bytes" at the literal instead of via a runtime
conversion call. No behavior change.
