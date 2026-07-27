---
"moadim": patch
---

feat(routines): pre-clone/cache declared repositories into the workbench before the agent launches (#466), instead of only printing them as a "clone any you need" prompt preamble. Each declared repository is backed by a persistent local mirror under `~/.config/moadim/cache/`, keyed by its URL — the first run clones the mirror, every later run of any routine referencing the same URL only `git fetch`es it, so repeated fires reuse already-downloaded objects instead of re-cloning from the remote. A clone/fetch failure (bad URL, unreachable host, a `branch` that doesn't exist upstream) now aborts the run with the reason recorded in `agent.log`, the same as the existing prompt-copy/setup/tmux guards, instead of failing silently inside the agent session.
