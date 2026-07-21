---
"moadim": patch
---

Fix stale `ui/src/*.rs` doc-comment references in `client/src/`. `bc9da2e` removed the legacy Yew UI crate (`ui/`) in favor of the React client, but left ~28 doc comments across `client/src/` pointing contributors at now-deleted Rust files as the "ported from" / "see that file for reference behavior" source of truth. Marked each stale reference `(removed)`, and reworded `heatmapMath.ts`'s comment (which told the reader to go check the deleted file for reference behavior) to note the port is now the sole implementation. No behavior change; doc comments only.
