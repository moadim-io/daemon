---
"moadim": patch
---

### Removed

- Removed the vestigial `echo` demo endpoint/tool — the scaffold `POST
  /api/v1/echo` REST route, the `echo` MCP tool, and their `EchoRequest` /
  `EchoResponse` / `EchoInput` types and OpenAPI entries, plus the `moadim
  echo <message>` CLI passthrough. It echoed a message back with a server
  timestamp, served no product purpose, and only widened the REST + MCP +
  OpenAPI + CLI surface; `GET /health` already covers liveness probing. The
  committed `apis/openapi.json` is regenerated without the `/echo` path and
  schemas (#359).
