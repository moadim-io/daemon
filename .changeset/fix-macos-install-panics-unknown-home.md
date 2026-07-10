---
"moadim": patch
---

fix(service): `install`/`uninstall` return an error instead of panicking on macOS when `$HOME` is undeterminable

The macOS launchd backend's `install()` and `uninstall()` both `.expect()`-ed the home directory
lookup, crashing the whole process with a panic if the home directory couldn't be resolved (e.g.
`$HOME` unset and no passwd entry, such as some minimal service/CI contexts). `plist_path_from_home`
already turns that condition into a proper `anyhow::Error` — the callers just weren't using it. Now
both functions propagate the error via `?`, matching the Linux systemd backend's `install()`/
`uninstall()`, which already propagate their equivalent `unit_path()` lookup the same way. No
behavior change on the happy path.
