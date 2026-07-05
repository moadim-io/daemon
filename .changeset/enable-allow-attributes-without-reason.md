---
"moadim": patch
---

chore(lint): enable `clippy::allow_attributes_without_reason`

Every `#[allow(...)]`/`#![allow(...)]` suppression now carries a `reason = "..."`
so a reviewer can tell at a glance why the lint was silenced, and the reason
stays documented as the code evolves. Fixed the handful of bare allows this
newly-`deny`d lint caught (mostly `#![allow(clippy::missing_docs_in_private_items)]`
at the top of test files, plus a `too_many_arguments` and a `zombie_processes`
allow).
