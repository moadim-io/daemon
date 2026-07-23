---
"moadim": patch
---

feat(client): command palette covers every page and can trigger a routine inline

The `⌘K` command palette was missing the Reliability and Machines pages, and
selecting a routine only opened the generic Routines list instead of that
routine. It now lists all six pages, deep-links a selected routine straight
to its history (reusing the existing `?history=<id>` link), and adds a
`⌘⏎`/⚡-button quick action to trigger a routine without leaving the palette.
