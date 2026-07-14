---
"moadim": patch
---

test(ui): cover the UI's `humanize_bytes` byte-formatting helper

`routines::model::humanize_bytes` (used by the cleanup toast) mirrors the CLI's own
`humanize_bytes` (`src/cli/query.rs`, tested by `src/cli/cleanup_bytes_tests.rs`) byte-for-byte,
but had zero unit tests of its own — the `ui` crate isn't held to the root package's 100%
line-coverage floor, so this pure, deterministic function silently had no regression net despite
its CLI twin being fully covered. Adds the same edge cases the CLI test already exercises (sub-KB,
each unit boundary, MB-range rounding, and the u64::MAX TB cap) so a future edit that de-syncs the
two implementations' output fails a test instead of only showing up as a visual mismatch between
`moadim cleanup`'s CLI output and the UI's cleanup toast. No behavior change.
