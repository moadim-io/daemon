---
"moadim": patch
---

Add `GET /routines/{id}/prompt-preview` (and the matching `preview_routine_prompt` MCP tool) to return the exact composed prompt body a routine's run would receive, computed in-memory with no workbench, `prompt.md` write, or agent launch. Lets operators verify repo clone bullets and prompt composition before a scheduled or manual run consumes a workbench.
