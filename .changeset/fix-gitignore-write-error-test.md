---
"moadim": patch
---

Fix `write_routine_fails_on_gitignore_write_error` to actually exercise the `.gitignore` write-failure branch it claims to cover, instead of accidentally failing one line earlier on the `prompts/` subdir creation.
