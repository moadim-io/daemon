---
"moadim": patch
---

refactor(ui): extract inline styles from index.html to styles.css

Moves the app's CSS out of a 1600+ line inline `<style>` block in
`ui/index.html` into `ui/styles.css`, linked via trunk's
`data-trunk rel="css"` asset pipeline. The self-hosted font-face
data-URI stays inline.
