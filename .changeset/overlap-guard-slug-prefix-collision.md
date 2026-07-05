---
"moadim": patch
---

### Fixed

The overlap guard (#514) matched a live tmux session by a plain string prefix (`moadim-<slug>-`), so a routine whose slug is itself a prefix of another routine's slug (e.g. `deploy` vs. `deploy-staging`) could have its own fire silently skipped by an unrelated routine's session — `"moadim-deploy-staging-<rid>".starts_with("moadim-deploy-")` read as "deploy is still running" even when deploy had no session of its own. The match now requires the text after the prefix to have the exact `$RID` shape the launcher emits (`<unix-ts>_<pid>`), so only a genuine fire of the same routine counts.
