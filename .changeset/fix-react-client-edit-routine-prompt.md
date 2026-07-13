---
"moadim": patch
---

fix(client): edit-routine form no longer shows a blank prompt

`GET /routines` omits each routine's `prompt` by default (it's the largest field and rarely
needed in a listing). The React client's edit modal built its initial form values straight from
that cached list row, so the prompt textarea always opened empty and the Save button stayed
disabled until the user retyped the whole prompt. The edit modal now fetches the single routine
by id (`GET /routines/{id}`, which always includes the prompt) when it opens, showing a spinner
until it loads.
