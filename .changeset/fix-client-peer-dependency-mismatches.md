---
"moadim": patch
---

fix(client): realign `@vitejs/plugin-react`, `react-dom`, and `@types/react-dom` with their peer dependencies

Two prior automated dependency bumps left `client/` with unsatisfiable peer
dependencies: `@vitejs/plugin-react@6.0.3` requires `vite@^8.0.0` (client is
pinned to `vite@^6.0.5`), and `react-dom@19.2.7`/`@types/react-dom@19.2.3`
require `react@^19`/`@types/react@^19` (client is pinned to the `18.3.x`
line). The first broke `vitest` at config-load time
(`ERR_PACKAGE_PATH_NOT_EXPORTED` resolving `vite/internal`); the second, once
unmasked, crashed every test that actually rendered a component
(`react-dom-client.development.js` reading an undefined internal field).
Together they meant `pnpm --filter client test` — part of both CI's
`client-test` job and the local pre-push hook — could not run at all.

Pins `@vitejs/plugin-react` to `^5.2.0` (last major compatible with
`vite@^6`) and `react-dom`/`@types/react-dom` back to the `18.3.x` line
matching `react`/`@types/react`. No application code changed; `pnpm --filter
client test` (329 tests), `typecheck`, and `build` are all green again.
