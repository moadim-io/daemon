---
"moadim": patch
---

fix: attach pipe-pane atomically with tmux new-session

`pipe-pane` was attached as a separate statement after `new-session -d`, so
any output the agent emitted before the attach (banner, initial plan,
startup crash) was silently dropped from `agent.log`. Both are now chained
in a single `tmux` invocation via `\;`, so the pipe is attached before the
pane can produce output.
