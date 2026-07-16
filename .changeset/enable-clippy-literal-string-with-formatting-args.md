---
"moadim": patch
---

chore(lint): enable `clippy::literal_string_with_formatting_args` workspace-wide

Denies a string literal that looks like a `format!`-family placeholder (`"{name}"`) sitting
outside a formatting macro — usually a leftover `format!`/`println!` argument that got moved
into a plain string and silently stopped interpolating. Surfaced 2 violations in
`routines/command.rs::substitute`, both intentional `String::replace` placeholder tokens rather
than formatting-macro arguments; annotated with a scoped `#[allow(reason = ...)]` explaining why.
No behavior change.
