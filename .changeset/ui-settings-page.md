---
"moadim": minor
---

feat(ui): add a Settings page for the persistent agent prompt

`~/.config/moadim/user_prompt.md` — the prompt text appended to every
routine's agent instructions file — was previously editable only by hand
on disk. `GET`/`PUT /config/user-prompt` now expose it over the REST API,
and a new SETTINGS page (nav tab + command palette entry) lets it be
viewed and edited from the UI. Machine identity and the global schedule
lock keep their existing header/banner controls; this page covers the one
setting that had no UI surface at all.
