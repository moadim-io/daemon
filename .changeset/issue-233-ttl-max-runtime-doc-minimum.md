---
"moadim": patch
---

docs(routines): note the `> 0` requirement on `ttl_secs`/`max_runtime_secs`

`svc_create`/`svc_update` already reject `ttl_secs: 0` and `max_runtime_secs: 0`
with `400 Bad Request` (#239), but the REST/MCP field docs (and the generated
OpenAPI spec) never said so, leaving the constraint undiscoverable to callers
until they hit the error. Documents the minimum on `Routine`,
`CreateRoutineRequest`, `UpdateRoutineRequest`, and the MCP `update_routine`
input, closing the last unchecked box on #233.
