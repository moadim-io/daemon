---
"moadim": patch
---

test(client): cover failureNotify.ts's localStorage error-handling branch

`pnpm --filter client test:coverage` showed `src/lib/failureNotify.ts`'s
`loadNotifyFailures` `catch` (falling back to notifications-off) untested,
even though it exists specifically to keep a blocked `localStorage`
(private browsing, quota exceeded, disabled storage) from crashing the
failure-notification preference read. Mirrors the same gap already closed
for `theme.ts` (see the `theme-storage-error-coverage` changeset): adds
tests that mock `Storage.prototype.getItem`/`setItem` to throw and assert
the fallback/no-throw behavior each catch is there for.

No behavior change — test-only. `failureNotify.ts` is now at 100% line
coverage.
