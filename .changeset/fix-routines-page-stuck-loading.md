---
"moadim": patch
---

fix(ui): routines page stuck loading, never fetches on mount

`RoutinesPage`'s mount-time fetch went through `install_routines_loader`, a helper
that wraps `use_effect_with` and gets invoked as a bare statement in the component
body. That effect never actually fired at runtime, leaving `state.loading` permanently
true and the routine list empty even though the API responded fine. Inlined the effect
directly into `page.rs`, matching the pattern the working Overview page already uses,
and removed the now-dead `install_routines_loader` helper. Also added
`RequestCache::NoStore` to the routines list fetch so a stale cached empty response
can't mask this class of bug again.
