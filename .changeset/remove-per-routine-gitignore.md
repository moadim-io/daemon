---
"moadim": patch
---

Stop generating a `.gitignore` inside every routine directory; the config dir's root `.gitignore` (now also seeding `*.compiled.*` and `run.sh`) covers routine dirs recursively, and any leftover per-routine `.gitignore` is removed on startup re-persist.
