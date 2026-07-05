---
"moadim": patch
---

fix(cli): unknown command exits 2 to stderr, not help to stdout

An unrecognized first argument (e.g. `moadim staus`) is no longer treated
as a successful `help` invocation. It now prints `unknown command: <arg>`
plus a hint to stderr and exits `2`, distinct from an explicit
`help`/`-h`/`--help` request (stdout, exit `0`).
