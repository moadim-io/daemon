---
"moadim": patch
---

### Fixed

- **The routine LOGS view search/highlight could panic the UI on ordinary Unicode log content.** `highlight()` found matches by lowercasing the whole line and then reapplying *those* byte offsets to the original (un-lowercased) string. Case folding isn't always byte-length-preserving (`ẞ`, U+1E9E, 3 bytes, lowercases to `ß`, U+00DF, 2 bytes) or even char-count-preserving (Turkish `İ` expands to two chars), so a matching search query after such a character could compute an offset that lands mid-character and panics on the slice (crashing the Yew render for that log line). The matching logic now projects each original char to exactly one lowercase char and tracks byte spans via `char_indices()`, so every slice boundary is guaranteed valid regardless of the log content's script.
