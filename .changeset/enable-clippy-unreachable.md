---
"moadim": patch
---

chore(lint): enable `clippy::unreachable` workspace-wide

Rejects the `unreachable!()` macro: a daemon process (or the running Yew UI) that hits one
panics instead of returning a structured error, and a match arm that looks provably impossible
today can become reachable after an innocuous refactor elsewhere — the panic then only surfaces
at runtime, in production. Enabled in both the root `Cargo.toml` and `ui/Cargo.toml` (the `ui`
crate has its own `[lints.clippy]` table and doesn't inherit root's deny-list), mirroring the
pattern of prior lint-parity chores.

Fixed the one violation this surfaced: `src/build/ui.rs`'s `base64_encode` matched on a 3-byte
chunk's length with a `_ => unreachable!()` catch-all for lengths other than 1/2/3 (impossible
from `bytes.chunks(3)`). Rewritten without a `match` — the guaranteed-present first byte is
indexed directly and the optional second/third bytes are read via `.get()` — so there's no
catch-all arm left to guard, and the output is unchanged. The rest of the workspace (including
`ui/src`) was already clean, so this locks that in with `deny`.
