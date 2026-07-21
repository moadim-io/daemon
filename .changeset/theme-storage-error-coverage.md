---
"moadim": patch
---

test(client): cover theme.ts's localStorage error-handling branches

`pnpm --filter client test:coverage` showed `src/lib/theme.ts` at 85.71%
lines: `loadThemeLight`'s `catch` (line 13, falling back to dark) and
`saveThemeLight`'s `catch` (swallowing the write failure) were both
untested, even though they exist specifically to keep a blocked
`localStorage` (private browsing, quota exceeded, disabled storage) from
crashing the theme toggle. Adds two tests that mock `Storage.prototype`'s
`getItem`/`setItem` to throw and assert the fallback/no-throw behavior each
catch is there for.

No behavior change — test-only. `theme.ts` is now at 100% coverage.
