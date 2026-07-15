---
"moadim": patch
---

chore(lint): enable `clippy::string_lit_as_bytes` in the `ui` crate. Mirrors the same lint
already enabled workspace-root-side (#1202) — the `ui` crate has its own `[lints.clippy]` table
with no `workspace = true` inheritance, so it was silently exempt despite `clippy --workspace`
covering it in CI. The `ui` crate was already clean, so no source changes were needed. No
behavior change.
