---
"moadim": patch
---

fix(client): pin the `@redocly/openapi-core` → `js-yaml` transitive dependency to a patched version

`openapi-typescript` (used by `client`'s `generate:api` script, which `build`/`typecheck`/`lint`/`test`
all run through their `pre*` hooks) pulls in `@redocly/openapi-core@1.34.17`, which resolves
`js-yaml@4.2.0` — a version affected by [GHSA-52cp-r559-cp3m](https://github.com/advisories/GHSA-52cp-r559-cp3m)
(quadratic CPU consumption via YAML merge-key chains), flagged `high` by `pnpm audit`.

Adds a scoped `pnpm.overrides` entry (`"@redocly/openapi-core>js-yaml": ">=4.3.0"`) rather than a
blanket `js-yaml` override, since the tree also carries `js-yaml@3.15.0` via `@changesets/cli`'s
`read-yaml-file` dependency — a global override would force that 3.x consumer onto an incompatible
4.x API. `@redocly/openapi-core` itself already declares `js-yaml: ^4.2.0`, so 4.3.0 satisfies its
own declared range; codegen output (`schema.gen.ts`), `pnpm --filter client typecheck`, `lint`, and
`test` (347 tests) are all unchanged. `pnpm audit` now reports no known vulnerabilities.
