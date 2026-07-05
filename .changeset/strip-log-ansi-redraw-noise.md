---
"moadim": patch
---

fix(routines): strip ANSI escapes and `\r`-redraw noise from served logs

`tmux pipe-pane -o` captures a routine's pane output verbatim, so `GET
/routines/{id}/logs`, the run-detail log endpoint, and the `logs` MCP tool
all served raw terminal escape sequences (color codes, cursor movement,
screen clears) and every redraw frame of a spinner or progress bar as its
own line, instead of the final state a real terminal would display (#278).
`read_log_tail` now strips ANSI/VT escape sequences and collapses
`\r`-based redraw overwrites down to the last write per line before
returning content, so served logs read as the logical lines an operator
would actually see in a terminal.
