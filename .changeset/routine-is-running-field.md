---
"moadim": minor
---

feat(routines): surface `is_running` on `GET /routines`/`GET /routines/{id}`

Adds a derived, non-persisted `is_running: bool` field to the routine
response, reporting whether any fire of the routine currently has a live
tmux session. Reuses the existing overlap-guard tmux-prefix probe
(`tmux_session_prefix_alive`, #514) that `svc_trigger` already relies on, so
an operator (or the UI, in a follow-up) can finally tell "is this routine
running right now?" from `GET /routines` instead of shelling in to `tmux ls`.
