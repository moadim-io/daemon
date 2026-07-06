---
"moadim": patch
---

test(routines): cover `next_run_at`'s "no future fire" branch in `model.rs`

`next_run_at`'s doc comment documents three `None` cases — disabled, an
unparseable schedule, and a schedule with no upcoming fire — but only the
first two had tests. `cargo llvm-cov`'s region report showed the third
branch (`cron.iter_after(Local::now()).next()?` returning `None`) was never
exercised. Added a test using a parseable 7-field (`sec min hour dom month
dow year`) schedule pinned to a year that has already passed, so parsing
succeeds but the iterator yields no occurrence.

No behavior change — regression test only.
