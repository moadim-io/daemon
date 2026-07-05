---
"moadim": patch
---

feat(ui): add TAG filter to the routines filter bar and include tags in search

Two improvements to tag-based visibility:
- Tags are now included in the free-text search haystack, so typing a tag name
  into the search box narrows the list to routines carrying that tag.
- When any routines have tags, a TAG drop-down appears in the filter bar,
  allowing operators to filter the table to a single tag without using the
  search box. The drop-down is hidden when no routines are tagged.
