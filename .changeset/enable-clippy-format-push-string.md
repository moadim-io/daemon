---
"moadim": patch
---

chore(lint): enable `clippy::format_push_string`

Fixes the 3 existing violations in `compose_prompt` (`src/routines/command.rs`), which built a repository/flag line with `format!` only to immediately copy it into the routine's prompt body via `push_str` — an unnecessary throwaway `String` allocation per line. Switches those to `write!`/`writeln!` directly into the existing buffer, and enables `format_push_string = "deny"` to lock in the zero-violation state going forward. No behavior change.
