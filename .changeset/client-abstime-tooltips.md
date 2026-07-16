---
"moadim": patch
---

Fix the React client (`client/`) silently dropping the absolute-timestamp hover tooltip that the
original Yew UI (`ui/`) shows next to every relative "N ago" time. `ui/src/cron_utils.rs`'s
`abstime` had no TypeScript port at all, so the "STARTED"/"UPDATED" cells in
`RecentRunsTable.tsx`/`RoutineRow.tsx` rendered no `title`, `RoutineHistory.tsx`'s run-row title
carried only the workbench name, and `RunHistorySparkline.tsx`'s per-tick tooltip omitted the
absolute time — all despite each file being documented as a "direct port" of its Rust
counterpart. Adds `abstime` to `client/src/lib/cronUtils.ts` (mirroring the Rust formatting and
its zero/out-of-range fallbacks) and wires it into the four call sites so hovering a relative time
in the React client shows the same wall-clock timestamp the Yew UI has always shown.
