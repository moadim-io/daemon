---
"moadim": patch
---

chore(lint): enable clippy::exit workspace-wide, forbidding `std::process::exit`/`std::process::abort` outside `fn main`. Prevents a `Drop`-skipping process termination (leaked lock guards, file handles, in-flight routine cleanup) from a long-running daemon code path other than the CLI's top-level dispatch. Codebase was already clean; no fixes needed.
