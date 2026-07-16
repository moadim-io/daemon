---
"moadim": patch
---

chore(lint): enable `clippy::large_stack_arrays` workspace-wide

Denies a local array literal over 512000 bytes — a value that size belongs on the heap
(`Vec`/`Box`), not the stack. A daemon process runs long-lived worker threads with a fixed,
comparatively small stack, so an oversized stack array is a latent stack-overflow risk that only
surfaces under the right call depth, unlike a heap allocation which fails safely. The codebase was
already clean, so this surfaced 0 violations. No behavior change.
