---
"moadim": patch
---

feat(ui): show open flag count in NEEDS ATTENTION detail column

The NEEDS ATTENTION panel now shows "N open flag(s) — needs review"
instead of a generic detail string for HasOpenFlags rows, so operators
can see the severity at a glance without navigating into the routine.
AttentionItem now carries flag_count; a new test verifies it is
correctly propagated from SchedSource.
