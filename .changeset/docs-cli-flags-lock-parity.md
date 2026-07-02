---
"moadim": patch
---

Correct the README and `commands.rs` module doc, which claimed the CLI exposes "every" routine action the REST API and MCP tools do — routine flags (`create_flag`/`list_flags`/`resolve_flag`) and the global routine lock (`get_lock_status`/`lock_routines`/`unlock_routines`) have no `moadim` subcommand and are REST/MCP-only.
