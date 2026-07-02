---
"moadim": patch
---

### Fixed

- **Corrected the `stop_json` doc comment's stale claim.** It said `stop --json`'s shape matches
  `status --json` "exactly", but `status --json` later gained `uptime_secs`/`version` fields that
  `stop --json` never got — the two shapes are a subset relationship (already enforced by
  `status_and_stop_json_share_a_common_key_set`), not an exact match. Doc-only; no behavior change.
