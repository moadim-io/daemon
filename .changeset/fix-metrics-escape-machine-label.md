---
"moadim": patch
---

fix(observability): escape the `machine` label in `GET /api/v1/metrics`'s `moadim_build_info` series for Prometheus text-exposition syntax. `machine` is the only label value in `src/routes/metrics.rs` sourced from free-form user input (`moadim machine set <name>` / `MOADIM_MACHINE`, trimmed but otherwise unrestricted — see `src/machine/mod.rs`); an operator-chosen name containing `"` or `\` was emitted into the label literally, producing unparseable exposition text that would fail the whole scrape, not just that one line. Adds `escape_label_value` (backslash, double-quote, and newline escaping per the exposition format) and applies it to `machine`; `version`/`git_sha` are compile-time constants and don't need it.
