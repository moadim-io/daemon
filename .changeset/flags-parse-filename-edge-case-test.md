---
"moadim": patch
---

### Tests

- Added a `flags_tests` regression guard
  (`list_flags_skips_md_files_that_dont_match_the_flag_shape`) covering two
  `parse_filename` edge cases that were previously untested: a `.md` file with
  no `-` to split a timestamp off of (e.g. `README.md`), and a `.md` file
  whose `-`-delimited suffix isn't a valid timestamp (e.g.
  `bug-notatimestamp.md`). `list_flags` is documented to silently skip
  unparsable filenames rather than error; this locks that contract in against
  a future regression (e.g. someone swapping the `?` for `.unwrap()` in
  `parse_filename`).
