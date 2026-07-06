---
"moadim": patch
---

### Fixed

Warn at startup when the server binds to a non-loopback address. The REST/MCP API has no authentication (#504), so exposing it beyond `127.0.0.1`/`::1` grants anyone who can reach that address unauthenticated routine CRUD; the daemon now logs a loud warning at launch, matching the existing tmux/python3 startup checks, instead of leaving this risk silent.
