---
"moadim": patch
---

Enable `clippy::match_same_arms` and merge the two duplicate-body arms it flagged in `cli::parse` (issue #719): the bare `None` arm into the `Background` arm, and the redundant explicit `-h`/`--help`/`help` arm that the trailing wildcard already covered.
