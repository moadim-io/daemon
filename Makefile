# Convenience wrappers around tools the pre-commit/pre-push hooks and CI
# already enforce, so contributors don't need to know a tool's crate/binary
# name to run the same check locally. Keep in lockstep with .githooks/ and
# .github/workflows/ — this only wraps the existing gates, it never redefines
# them.

.PHONY: spell

spell:
	@command -v typos >/dev/null 2>&1 || cargo install typos-cli
	typos
