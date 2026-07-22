---
"moadim": patch
---

test(client): cover `unwrapVoid`'s generic-message fallback branch

`pnpm --filter client test:coverage` showed `src/api/client.ts` at 100% lines but only 92.85% branches, missing `unwrapVoid`'s `error?.error ?? \`HTTP ${response.status}\`` fallback — the sibling `unwrap` function already had a test for this exact branch ("throws a generic message when the error body has no message"), but `unwrapVoid` never got the matching one. Adds it, mirroring the existing `unwrap` test one-for-one.

No behavior change — test-only. `src/api/client.ts` is now at 100% branch coverage.
