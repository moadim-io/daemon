---
"moadim": patch
---

Fix flags being written to and read from a phantom title-slug directory for routines that live in a folder or have been renamed/moved, by keying all flag operations (create, list, resolve) and `flag_count` on the routine's actual on-disk relative directory instead of `slugify(title)`. On first start, flags already stranded in such a title-slug directory are moved into the routine's real directory (a flag whose name already exists there is left in place), so none disappear.
