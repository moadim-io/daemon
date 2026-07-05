---
"moadim": patch
---

fix(agents): enable network in the codex default so unattended routines can reach the remote

`codex exec` runs unattended (no approval prompts), but its default
`workspace-write` sandbox blocks outbound network access, so a routine could
not clone the repo or push / open a PR. The built-in codex default now pins
the sandbox to `workspace-write` explicitly and turns network access back on
— the least-privilege setting that still lets the routine reach the remote,
mirroring the baseline the `claude` default already gets.
