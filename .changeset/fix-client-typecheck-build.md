---
"moadim": patch
---

fix(client): restore a working `client/` TypeScript build

`typescript` was bumped to `^7.0.2` (a pre-release/native-compiler major), but `openapi-typescript`
(which `generate:api` runs before every `typecheck`/`lint`/`test`/`build` script) declares a peer
dependency of `typescript: "^5.x"` and crashes immediately (`ts.factory` is `undefined`) under 7.x.
That single crash was tripping `pretypecheck`/`prelint`/`pretest` before those scripts ever ran,
so every PR's `client (typecheck + lint)` and `client (vitest)` CI jobs have been red since the
bump landed. `typescript` is pinned back to `^5.9.3`, the last version compatible with
`openapi-typescript`'s peer range.

With `generate:api` unblocked, `tsc --noEmit` surfaced two more breaks from unrelated dependency
bumps that had been landing behind the same crash: `cron-parser`'s v5 major dropped the
`parseExpression` named export in favor of the `CronExpressionParser.parse()` static method, and
`react-router-dom`'s v7 major removed the `future` prop entirely (its `v7_startTransition`/
`v7_relativeSplatPath` flags are now always-on defaults). Both call sites are updated to match.

`tsc --noEmit` is clean again. Out of scope for this patch (separate, pre-existing dependency-bump
regressions, unrelated to anything touched here): `eslint-plugin-react-hooks`'s new major flags
`react-hooks/set-state-in-effect` at several existing call sites, and `@vitejs/plugin-react`'s
6.x major wants `vite@^8` while the workspace still pins `vite@^6`, which crashes `vitest`'s config
load before any test runs.
