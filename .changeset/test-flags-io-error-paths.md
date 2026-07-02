---
"moadim": patch
---

### Tests

- **Covered `routines::flags`'s I/O error paths.** `create_flag`,
  `list_flags`, and `resolve_flag` each have a filesystem-error branch
  (a failed `create_dir_all`, `read_to_string`, or `remove_file`) that
  `cargo llvm-cov` region coverage showed had zero executions. Added three
  tests exercising each: `create_flag_propagates_create_dir_failure`,
  `list_flags_skips_entries_it_cant_read_as_text`, and
  `resolve_flag_propagates_remove_failure`. No behavior change — this locks
  the existing error handling in against a future regression.
