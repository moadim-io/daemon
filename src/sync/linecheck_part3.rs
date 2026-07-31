
/// Read the current user crontab via `crontab -l`.
///
/// Returns an empty string when no crontab exists for the user.
pub(crate) fn read_crontab() -> Result<String, SyncError> {
    let out = Command::new(crontab_bin())
        .arg("-l")
        .output()
        .map_err(|err| SyncError::CrontabCommand(format!("failed to run crontab -l: {err}")))?;

    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }
    // "no crontab for <user>" is a normal condition — treat as empty.
    let stderr = String::from_utf8_lossy(&out.stderr);
    if stderr.contains("no crontab") {
        return Ok(String::new());
    }
    Err(SyncError::CrontabCommand(stderr.into_owned()))
}

/// Install `content` as the user's crontab via `crontab -`.
pub(crate) fn write_crontab(content: &str) -> Result<(), SyncError> {
    let mut child = Command::new(crontab_bin())
        .arg("-")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|err| SyncError::CrontabCommand(format!("failed to spawn crontab: {err}")))?;

    // Taking stdin drops (closes) it after write_all, signalling EOF. A write
    // failure (e.g. the child exits early — a strict `crontab` rejecting
    // malformed input mid-stream — and closes its end of the pipe) is a real,
    // externally-triggerable I/O condition, not a programmer error, so it is
    // propagated as `SyncError::Io` instead of panicking: every caller of
    // crontab sync already treats a `SyncError` as warn-and-continue (see the
    // module docs), and a panic here would defeat that graceful degradation.
    #[allow(
        clippy::expect_used,
        reason = "`.stdin(Stdio::piped())` is set on this same `Command` a few lines above, so \
                  `take()` returning `None` here would mean the stdlib itself broke that \
                  contract, not a real runtime error"
    )]
    let write_result = child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(content.as_bytes());

    // Always wait to reap the child, even when the write above failed. Use a
    // timeout instead of plain `wait()`: on macOS, privacy/TCC prompts for
    // `SystemPolicySysAdminFiles` can leave setuid `crontab -` blocked
    // headlessly, which otherwise wedges daemon startup before HTTP binds.
    let status = wait_for_crontab_write(&mut child)?;
    write_result?;

    if !status.success() {
        return Err(SyncError::CrontabCommand(format!(
            "crontab - exited with {status}"
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod sync_tests;

#[cfg(test)]
#[path = "crontab_io_tests.rs"]
mod crontab_io_tests;

#[cfg(test)]
#[path = "clear_crontab_tests.rs"]
mod clear_crontab_tests;

#[cfg(test)]
#[path = "mod_replace_block_tests.rs"]
mod replace_block_tests;
