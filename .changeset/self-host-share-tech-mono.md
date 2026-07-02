---
"moadim": patch
---

Self-host the `Share Tech Mono` webfont (base64-embedded `@font-face` in `ui/index.html` / `prebuilt.html`) instead of fetching it from `fonts.googleapis.com`/`fonts.gstatic.com` at runtime. The served UI now renders offline, with no third-party requests on load, and no FOUT while the CDN round-trip completes (#467). Font is SIL OFL 1.1 licensed; see `ui/assets/share-tech-mono.OFL.txt`.
