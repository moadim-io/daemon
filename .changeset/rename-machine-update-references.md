---
"moadim": minor
---

When renaming this machine via `PUT /api/v1/machine`, automatically update all routines that targeted the old name: replace the old machine name with the new one in each routine's `machines` list, persist the changes to disk, and re-sync the crontab. Previously only the machine identity file was updated, leaving routines orphaned on the renamed machine until each was manually edited.
