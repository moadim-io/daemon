---
"moadim": patch
---

Crontab block replacement now matches its delimiters as whole lines instead of raw substrings, guarding against a marker prefix-matching a more specific one elsewhere in the crontab and silently overwriting it. (#324)
