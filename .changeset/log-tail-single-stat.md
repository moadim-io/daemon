---
"moadim": patch
---

fix(routines): stat `agent.log` once when reading its tail with metadata

`read_log_tail_with_meta` (backing the MCP `routine_logs` tool and the HTTP
logs route) stated the log file twice: once for `total_bytes`/`truncated`,
then again inside `read_log_tail` to size the actual read. For a log still
being appended to by a live `tmux pipe-pane` capture, the file could grow
between those two stats, so the reported `total_bytes`/`truncated` could
describe a different moment in time than the `content` actually returned.
Both callers now share a single stat, so the metadata always matches the
content it describes.
