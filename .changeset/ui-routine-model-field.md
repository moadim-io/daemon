---
"moadim": patch
---

Add a **Model** field to the routine create/edit form and Routines table row. The backend already persisted an optional `model` override per routine and passed it to the agent invocation as `--model` (`src/routines/model.rs`, `src/routines/command.rs`), but the UI never exposed it — this wires up the missing free-text input, save/clear round-trip, and row display (#742).
