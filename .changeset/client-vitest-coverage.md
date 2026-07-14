---
"moadim": patch
---

test(client): add Vitest coverage reporting (`pnpm --filter client test:coverage`). The `src/` and `ui/` crates already have a 100%-line-coverage CI gate, but `client/` (the newer React/TypeScript dashboard) had no coverage instrumentation at all. This adds a non-gating `v8` coverage report so gaps are visible; no threshold is enforced yet.
