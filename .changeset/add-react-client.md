---
"moadim": minor
---

feat(client): add a new React/TypeScript web client, served at `/client` alongside the existing `ui/`

A ground-up redesign of the web dashboard in React + TypeScript + Vite, with full feature parity
to the Yew `ui/` SPA (Overview, Routines, Heatmap, Settings). Built as a single self-contained
`dist/index.html` via `vite-plugin-singlefile` and embedded into the binary at compile time
(`src/build/client.rs`), mirroring `ui/`'s `prebuilt.html` pipeline. Served at `GET /client` (with
its own `/client/*` SPA fallback) purely additively — `ui/` at `/` is unchanged and still the
default. This is the first step of a planned rollout that will eventually retire `ui/`.
