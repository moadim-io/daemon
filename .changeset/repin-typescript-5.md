---
"moadim": patch
---

fix(client): re-pin `typescript` to `^5.9.3`, undoing the dependabot `^7.0.2` bump (#1311) that broke `generate:api` again (`openapi-typescript` crashes with `ts.factory` undefined under TypeScript 7, the same regression already fixed once in #1250). This was blocking every `client (typecheck + lint)` and `client (vitest)` CI job on main.
