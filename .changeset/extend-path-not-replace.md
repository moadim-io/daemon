---
"moadim": patch
---

fix(routines): extend PATH in run.sh instead of replacing it

The exported `PATH` in a routine's generated launch script now appends the
curated fallback dirs to the login shell's `$PATH` (`export PATH=$PATH:...`)
instead of replacing it outright. Version-manager shim dirs (nvm/pyenv/asdf/volta)
that the profile prepends now survive, so the agent resolves the node/python
the user actually selected; the curated dirs still guarantee `tmux` and the
agent command stay resolvable as a fallback.
