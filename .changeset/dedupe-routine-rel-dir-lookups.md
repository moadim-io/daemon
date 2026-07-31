---
"moadim": patch
---

`RoutineResponse::from_routine` (built for every routine on each `GET /routines` poll) called `routine_rel_dir` three times per routine — once directly and once each inside `routine_slug`/`routine_folder` — and every call re-walked the whole `routines/` tree. It now computes the relative directory once and derives the slug/folder from it with pure string ops (`slug_from_rel_dir`/`folder_from_rel_dir`), cutting the list endpoint's filesystem walks by 3x with no behavior change.
