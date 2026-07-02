---
"moadim": patch
---

Fix the routines UI failing to load with `missing field \`prompt\`` (#849) by adding `#[serde(default)]` to `Routine::prompt`, matching the server's `GET /routines` response which omits `prompt` by default since #825.
