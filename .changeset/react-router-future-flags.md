---
"moadim": patch
---

Opt the client's `BrowserRouter` (and the `MemoryRouter` used in `App.test.tsx`) into React Router's `v7_startTransition` and `v7_relativeSplatPath` future flags, silencing the two v7-upgrade warnings React Router logs on every render and test run. No behavior change.
