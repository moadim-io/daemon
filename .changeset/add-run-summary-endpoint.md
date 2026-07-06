---
"moadim": minor
---

Add `GET /routines/{id}/runs/{workbench}/summary`, serving an agent-authored work summary (`summary.md`) for a specific run. Every routine's system prompt now instructs the agent to keep a running work log and write a final summary section to that file before exiting.
