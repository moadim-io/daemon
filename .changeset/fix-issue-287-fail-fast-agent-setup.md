---
"moadim": patch
---

fix(routines): fail-fast on a failed agent `setup` step (#287)

A failing `setup` step (e.g. a trust/onboarding pre-seed script) was silently ignored — the
statements are `;`-joined with no `set -e` — so the agent launched anyway, typically hanging on an
interactive prompt with no stdin until the watchdog reaped it roughly an hour later with no
diagnostic. The setup step is now wrapped in a guard that aborts the launch and records the failure
in `agent.log` and on stderr, mirroring the existing `cp prompt.md` guard.
