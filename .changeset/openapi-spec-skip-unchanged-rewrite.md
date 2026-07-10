---
"moadim": patch
---

fix(server): skip rewriting the on-disk openapi spec when it hasn't changed

`write_openapi_spec` already skipped the write when `apis/`'s parent directory is
absent (the installed-binary case). It still rewrote the file unconditionally on
every dev startup even when the freshly generated spec was byte-for-byte identical
to what's already on disk, needlessly bumping the committed file's mtime. It now
compares against the existing contents first and skips the write when unchanged (#319).
