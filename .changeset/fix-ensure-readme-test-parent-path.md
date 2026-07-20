---
"moadim": patch
---

test(cli): fix `ensure_readme_returns_early_when_the_path_has_no_parent` to actually exercise its named branch. `Path::new("this-file-should-not-exist").parent()` is `Some("")`, not `None`, so the old test never hit `ensure_readme`'s `parent_or_err` early-return arm — it silently fell through `create_private_dir_all("")` (a no-op) into `std::fs::write`, dropping a stray `this-file-should-not-exist` file into the process's current directory on every `cargo test` run (reproduced: present, untracked, and un-gitignored at the repo root). Switched the test to `Path::new("")`, one of the few paths whose `.parent()` is genuinely `None`, so the branch is actually covered and the run no longer leaves a stray file behind. No production code change.
