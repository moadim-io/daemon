use super::DefaultRoutine;

/// Built-in spec for the daily `moadim` cargo package update routine.
pub(super) const SPEC: DefaultRoutine = DefaultRoutine {
    title: "Update moadim cargo package",
    // Daily at 09:00 local time.
    schedule: "0 9 * * *",
    agent: "claude",
    prompt: PROMPT,
};

/// Task prompt handed to the agent.
const PROMPT: &str = "\
Ensure the locally installed `moadim` cargo package is up to date, and update it if it is not.

Steps:
1. Find the installed version: `cargo install --list | grep '^moadim '` (no output means it is not installed).
2. Find the latest published version on crates.io: `cargo search moadim --limit 1`.
3. If `moadim` is not installed, or the installed version is older than the latest published version, run `cargo install moadim --force` to update it.
4. If it is already on the latest version, make no changes.

Report which versions you found and whether an update was performed.
";
