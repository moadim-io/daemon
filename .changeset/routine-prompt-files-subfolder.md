---
"moadim": patch
---

### Changed

- **A routine's prompt no longer lives inside `routine.toml`.** The raw prompt
  is now stored in its own file, `prompts/prompt.pure.md`, and the composed
  prompt (repositories preamble + raw prompt) moved from the top-level
  `prompt.md` to `prompts/prompt.compiled.md` — both inside the routine's
  directory. Embedding a long, often multi-line prompt as an escaped TOML
  string made `routine.toml` awkward to diff and edit; giving the raw prompt
  its own markdown file finishes the split the daemon already started for the
  composed prompt. Existing installs are migrated automatically on the next
  startup.
