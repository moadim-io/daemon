---
"moadim": patch
---

### Fixed

- **Structurally guarded the routine-launch `sh` spawn against test builds.**
  `spawn_routine_command` invoked `Command::new("sh")` directly, isolated in
  tests only by convention (clearing `PATH`), unlike the `crontab_bin()` seam
  (#175). A future test that triggers a routine without clearing `PATH` could
  execute a real login shell — and thus a real agent launch — on the
  developer's machine. Added `sh_bin()`, mirroring `crontab_bin()`: honors a
  `MOADIM_SH_BIN` override, and in test builds defaults to a nonexistent path
  when no override is set so the spawn fails harmlessly regardless of `PATH`
  state. (#217)
