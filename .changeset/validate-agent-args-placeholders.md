---
"moadim": patch
---

fix(routines): validate agent-config args placeholders

Creating or updating a routine now validates the referenced agent's `args`
against two silent fire-time failures (#322): a typo'd placeholder token
(e.g. `{prompt_fil}`) that would reach the agent as a literal, dead argument,
and a config with no `{prompt}`/`{prompt_file}` placeholder at all, which
would launch the agent with no task. Both are rejected with `400 Bad
Request` at edit time instead of silently burning a run at fire time.
