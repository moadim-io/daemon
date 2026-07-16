---
"moadim": patch
---

chore(lint): enable `clippy::many_single_char_names` workspace-wide

Rejects a scope with 4+ single-character bindings in play at once. Surfaced one violation:
`ui/src/routines/filter_tests.rs`'s `is_active_detects_each_facet` test had six (`q`, `s`, `a`,
`m`, `r`, `t`), one per `RoutineFilter` facet under test. Renamed to `query_filter`,
`status_filter`, `agent_filter`, `machine_filter`, `repo_filter`, `tag_filter`. No behavior change.
