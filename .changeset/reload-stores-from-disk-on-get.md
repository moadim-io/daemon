---
"moadim": major
---

feat(read): reload the routine store from disk on every GET (#774)

The daemon used to load `~/.config/moadim/routines/` once at startup into an in-memory cache; every `GET` (HTTP `/routines`, `/routines/{id}`, `/routines.ics`, and the equivalent MCP tools) served that stale snapshot, so config edits pulled into the directory — e.g. a routine's `machines` targeting list changing via `git pull` — stayed invisible until a daemon restart. `svc_list`/`svc_get`/the iCal feed now re-scan the on-disk routines directory and refresh the store before serving each request; disk is already the source of truth (every mutation persists before returning), so the reload-on-read loses no state, and the scheduler-written `last_scheduled_trigger_at` log is read back on every reload so it survives the refresh.
