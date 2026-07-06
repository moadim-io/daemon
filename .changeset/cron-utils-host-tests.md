---
"moadim": patch
---

test(ui): cover `parse_cron`/`describe_cron_live` in `cron_utils.rs`

`cron_utils.rs`'s field-count normalization (5-field passthrough, 6-field
with seconds, 7-field seconds+year stripping, `@keyword`, and invalid input)
and `describe_cron_live`'s validity/description pairing had no tests at
all, unlike the sibling `schedule.rs`/`schedule_heatmap.rs` pure-logic
modules which both have dedicated `*_tests.rs` files. Added
`cron_utils_tests.rs` following that same host-tested convention.
`reltime` is left untested — it calls `js_sys::Date::now()` and needs a
wasm/DOM host, mirroring the pure/DOM split already documented in
`refresh.rs`.

No behavior change — regression tests only.
