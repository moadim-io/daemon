---
"moadim": patch
---

feat(ui): surface dependency health warnings and build info in the header

Show "⚠ NO TMUX" (red, pulsing) and "⚠ NO PYTHON3" (amber) warning badges in
the header when the daemon reports a missing runtime dependency. Extends the
`Health` struct to include `dependencies` and `git_sha` from the existing
`/api/v1/health` response, and displays the git SHA as a tooltip on the version
label.
