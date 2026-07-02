---
"moadim": minor
---

### Added

- **`README.md` seeded into the config directory.** On every start, the daemon
  now writes `{config_dir}/README.md` (default `~/.config/moadim/README.md`)
  if it doesn't already exist, explaining the layout of `routines/`,
  `agents/`, and the daemon-managed files (`.gitignore`, `machine.local.toml`,
  `moadim.pid`, `daemon.log`) — so anyone who opens or git-tracks the config
  folder directly has an orientation doc without needing to consult the
  project README. Never overwrites an existing `README.md`, so user edits are
  preserved.
