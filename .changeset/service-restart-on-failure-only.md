---
"moadim": patch
---

fix(service): restart only on failure so a clean stop stays stopped

The systemd unit and launchd agent restarted the daemon on *any* exit
(`Restart=always` / unconditional `KeepAlive`), so a clean shutdown via
`moadim stop`, the UI STOP button, or `POST /shutdown` was resurrected by
the supervisor ~5s later. Restart is now failure-only
(`Restart=on-failure` / `KeepAlive = { SuccessfulExit = false }`): a crash
still auto-restarts, but a clean stop stays stopped.
