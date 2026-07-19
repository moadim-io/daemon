---
"moadim": patch
---

fix(client): resolve `RoutineForm`'s `react-hooks/incompatible-library` warning

`useForm().watch()` called with no arguments returns a subscription function
whose identity isn't stable across renders, so React Compiler's
`eslint-plugin-react-hooks` bails out of memoizing the whole component.
`RoutineForm` was the only place in the client tree using this pattern.

Replaced the bulk `watch()` call with scoped `useWatch({ control, name })`
calls for the five fields actually read (`title`, `schedule`, `agent`,
`prompt`, `machines`) — react-hook-form's recommended reactive-subscription
hook for this exact case. No behavior change; `pnpm --filter client lint`
now reports 0 warnings.
