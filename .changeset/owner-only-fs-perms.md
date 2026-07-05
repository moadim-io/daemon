---
"moadim": patch
---

fix(security): create the daemon's on-disk tree owner-only (#382)

The daemon's on-disk tree is a secret/transcript store (`agent.log` transcripts, `prompt.md`
instructions, token-referencing routine state) but was created at the default umask, landing
directories `0755` and files `0644` — world-readable on a default shared-host umask. Directories
under `~/.config/moadim/` are now created `0700` via `utils::fs_perms::create_private_dir_all`,
files published through `utils::atomic::atomic_write` (routine state, the `prompt.md` sidecar,
`machine.local.toml`) are now created `0600` before the publishing rename, and each routine's
launch script now sets `umask 077` before its first `mkdir` so the workbench directory and
everything written inside it — the copied `prompt.md`, appended `CLAUDE.md`, and tmux-piped
`agent.log` — stay unreadable by other local accounts. Unix-only; non-unix builds are unchanged.
Pre-existing files from older installs are tightened on their next write, not migrated
retroactively.
