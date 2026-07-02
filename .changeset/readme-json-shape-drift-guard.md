---
"moadim": patch
---

### Fixed

- **Pinned README `--json` shapes to actual CLI keys.** Added tests that parse
  the documented `status`/`cleanup`/`stop` `--json` shape literals straight out
  of `README.md` and assert they name exactly the keys the CLI emits, so a
  field rename, addition, or removal in `cli.rs` can no longer drift silently
  from the script-facing contract. (#345)
