---
"moadim": patch
---

### Fixed

- **`slugify` dropped every non-ASCII character.** Routine titles written in
  Hebrew, CJK, or Cyrillic (or Latin letters with diacritics like `é`/`ü`)
  slugified to an empty string and fell back to the generic `"routine"` name,
  so a second such routine collided on create (`409`) and the on-disk
  workbench dir / tmux session name gave no hint which routine it belonged
  to. `slugify` now uses `char::is_alphanumeric`/`char::to_lowercase` (Unicode
  scalar values, not ASCII-only), so non-Latin titles keep their content and
  two distinct non-Latin titles produce distinct slugs. (#262)
