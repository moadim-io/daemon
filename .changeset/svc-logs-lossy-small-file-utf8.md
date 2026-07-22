---
"moadim": patch
---

Fix `svc_logs` returning a 500 for a small `agent.log` containing invalid UTF-8 (e.g. binary tool output, or a multi-byte character split by a `tmux pipe-pane` write): the under-cap read path now uses a lossy UTF-8 decode, matching the truncated-tail path's existing behavior, instead of erroring out via `read_to_string`.
