---
"moadim": patch
---

chore(deps): bump `rmcp`/`rmcp-macros` to 2.2.0

Both stay within the `rmcp = "2.0.0"` (caret) requirement already declared in `Cargo.toml`, so
this is a `Cargo.lock`-only refresh — no manifest or code changes. The 2.1.0 -> 2.2.0 release
notes list only fixes (cancel-safe transport receive, refresh-token preservation, redirect
header-leak guard, unparsable-message handling, protocol version negotiation) and one addition
(rejecting auth servers lacking S256 PKCE support); no breaking changes. `cargo build`,
`cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` all pass
unchanged after the bump.
