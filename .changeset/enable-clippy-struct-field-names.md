---
"moadim": patch
---

Enable `clippy::struct_field_names` to reject a struct field whose name redundantly repeats the struct's own name. Fixes the one violation this surfaced: `Flag::flag_type` (`src/routines/flags.rs`) renamed to `Flag::category`, matching its doc comment ("Free-text category"). The wire format is unchanged — the field already carries `#[serde(rename = "type")]`. No behavior change.
