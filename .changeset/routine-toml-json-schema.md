---
"moadim": minor
---

feat(build): generate `routine.toml` JSON Schema + example

Generates `schemas/routine.schema.json` and `schemas/routine.example.toml` at build time from the
`RoutineToml` shape, mirroring the existing `job.schema.json` generation. Example TOMLs can reference
the schema via `#:schema ./routine.schema.json` for editor validation.
