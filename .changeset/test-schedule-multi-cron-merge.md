---
"moadim": patch
---

Add unit tests for `nextFireAfterAny`/`nextFiresAny` (client `src/lib/schedule.ts`), the multi-schedule "next fire" merge logic backing the Routines table row and the Overview page's dead-schedule detection. Both were previously untested (0% coverage on that code path).
