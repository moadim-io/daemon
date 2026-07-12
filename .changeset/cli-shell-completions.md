---
"moadim": minor
---

### Added

feat(cli): `moadim completions <shell>` prints a shell-completion script

Prints a completion script for `bash`, `zsh`, `fish`, `powershell`, or
`elvish` to stdout (e.g. `moadim completions zsh > _moadim`), covering the
lifecycle subcommands (`restart`, `stop`, `status`, `cleanup`, `trigger`,
`install`, `uninstall`, `machine`, `help`, `version`) and their `--json`/
`--quiet` flags. A missing or unrecognized shell prints a usage error to
stderr and exits non-zero, matching the rest of the CLI's error convention.

Generated via `clap_complete` from a small `clap::Command` built only for
this purpose — the CLI's existing hand-rolled argument parser is untouched
by this change (#307).
