---
"moadim": patch
---

### Fixed

Each routine's seeded `.gitignore` now also ignores `prompts/prompt.compiled.md` — the composed prompt is fully derived from `prompt.pure.md` + `routine.toml` and rewritten on every save, so it was getting tracked/committed even though it carries no information of its own (#1046). The pattern is reconciled into existing `.gitignore` files (not just newly created ones) the next time the daemon starts, alongside any other patterns a user has added.
