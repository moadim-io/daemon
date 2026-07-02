---
"moadim": minor
---

### Added

- **`README.md` seeded into the config directory and its generated
  subdirectories.** On every start, the daemon now writes a `README.md` into
  `{config_dir}` (default `~/.config/moadim/`), `{config_dir}/routines/`, and
  `{config_dir}/agents/` if one doesn't already exist there, explaining each
  folder's layout — the top-level file covers the daemon-managed files
  (`.gitignore`, `machine.local.toml`, `moadim.pid`, `daemon.log`), the
  `routines/` one covers the per-routine directory structure
  (`routine.toml`, `prompts/`, `flags/`, the `.local.` sidecars), and the
  `agents/` one covers the agent registry format. So anyone who opens or
  git-tracks the config folder directly has an orientation doc without
  needing to consult the project README. Never overwrites an existing
  `README.md`, so user edits are preserved.
