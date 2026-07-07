---
"moadim": patch
---

fix(routines): rename the compiled-prompt sidecar to prompt.compiled.local.md

`prompts/prompt.compiled.md` is fully derived from `prompt.pure.md` + `routine.toml`
and rewritten on every save, so it should never be tracked — but relying on an
explicit `.gitignore` entry (added in #1050) only stopped *new* writes from being
tracked; it did nothing for installs where the file had already been `git add`-ed
before that fix landed. Renamed it to `prompt.compiled.local.md` so it matches the
`*.local.*` pattern the same way `state.local.toml` does, and dropped the now-redundant
explicit `.gitignore` entry (#1046).

A new startup migration (`migrate_compiled_prompt_filename`) renames the file on disk
for existing routines. This does not touch git history or the index — the daemon has
no git integration — so an install with `prompt.compiled.md` already committed will
still need a manual `git rm --cached prompts/prompt.compiled.md` (or just let the next
commit record the rename) after upgrading.
