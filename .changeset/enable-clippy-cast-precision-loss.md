---
"moadim": patch
---

Enable `clippy::cast_precision_loss` to catch an `as`-cast from an integer to a floating-point type that can't represent every value of the source exactly (e.g. `u64 as f64`, past 2^52 the cast silently rounds to the nearest representable `f64`), the same "no indication of the invariant" gap `cast_sign_loss` and `cast_possible_wrap` already close for the other integer-cast directions. Fixes the 2 violations this surfaced: `cli::query::humanize_bytes`'s `u64` byte count cast to `f64` for display, which is deliberately an approximation already rounded to one decimal place and is kept as `as f64` behind a scoped, reasoned `#[allow]`; and a test's `delay_ms` shim parameter, narrowed from `u64` to `u32` and converted via `f64::from` so the cast is lossless by construction instead of merely lossless in practice. No behavior change.
