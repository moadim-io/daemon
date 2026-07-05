---
"moadim": patch
---

fix(service): enable systemd lingering so the daemon survives logout/reboot (#294)

On Linux, `moadim install` starts the daemon under the systemd *user* manager, but without
lingering enabled that manager — and `moadim.service` with it — only runs while the user has an
active login session, so the daemon stopped at logout and never started at boot. `install()` now
runs `loginctl enable-linger` and records a marker file so `uninstall()` disables it symmetrically,
without ever touching lingering the operator enabled themselves for an unrelated reason. Never
fails the install: if `loginctl` is unavailable or errors, a warning with the manual command is
printed instead of aborting.
