---
"moadim": patch
---

test(client): add coverage for `SettingsPage`'s persistent-prompt editor

`SettingsPage.tsx` had zero test coverage (`pnpm --filter client
test:coverage` showed 0% statements/branches/lines) despite carrying real
logic — seeding the draft from the loaded prompt, tracking dirty state, and
gating the Save button on it. Adds `SettingsPage.test.tsx` covering: the
loading state before the prompt query resolves, the textarea seeding from a
loaded prompt with Save disabled until edited, and Save enabling plus the
"unsaved changes" hint once the draft diverges from the loaded value.

No behavior change — test-only.
