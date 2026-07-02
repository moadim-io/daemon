---
"moadim": patch
---

### Tests

- Added a `cli_tests` regression guard (`status_and_stop_json_share_a_common_key_set`)
  asserting that every object key `stop --json` emits also appears in
  `status --json`, so the shared `{running,pid,address}` base contract between
  the two `--json` shapes can't silently drift apart as fields are added to
  one side but not the other.
