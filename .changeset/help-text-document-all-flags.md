---
"moadim": patch
---

docs(cli): document every accepted flag in `moadim --help`

The parser already accepted `-f`/`--foreground` as aliases for `-i`/`--interactive`,
`-d`/`--detach`/`--daemon` as aliases for `-b`/`--background`, and `--version` as a
long form of `-V`, but the help text never mentioned them — a user could only
discover these aliases by reading the source. The help text now documents every
alias the parser accepts, and a new test (`help_text_documents_every_accepted_flag`)
asserts the two can't silently drift apart again.
