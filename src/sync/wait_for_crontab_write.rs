#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

/// Wait for a `crontab -` child, killing it if the install does not finish promptly.
pub(crate) fn wait_for_crontab_write(child: &mut Child) -> Result<ExitStatus, SyncError> {
    let timeout = crontab_write_timeout();
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            let pid = child.id();
            let _ = child.kill();
            let status = child.wait()?;
            return Err(SyncError::CrontabCommand(format!(
                "crontab - timed out after {}s; killed pid {pid} ({status})",
                timeout.as_secs()
            )));
        }
        thread::sleep(CRONTAB_WAIT_POLL_INTERVAL);
    }
}

/// Resolve the configured timeout for installing crontab content.
pub(crate) fn crontab_write_timeout() -> Duration {
    std::env::var(CRONTAB_WRITE_TIMEOUT_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .map_or(DEFAULT_CRONTAB_WRITE_TIMEOUT, Duration::from_secs)
}

// ─── Block assembly ────────────────────────────────────────────────────────

/// Locate a delimiter line whose trimmed content is *exactly* `marker`.
///
/// Returns `(line_start, marker_end)` — the byte offset where the marker's line
/// begins and the offset just past the marker text — or `None` when no line
/// matches. Matching the marker as a whole line (rather than a raw substring)
/// keeps a prefix marker like `# BEGIN MOADIM` from matching the cron-jobs *and*
/// the more-specific routines marker `# BEGIN MOADIM-ROUTINES`, which would let
/// a cron-jobs sync silently overwrite the routines block (issue #324).
pub(crate) fn find_marker_line(crontab: &str, marker: &str) -> Option<(usize, usize)> {
    let mut offset = 0;
    for line in crontab.split_inclusive('\n') {
        let content = line.trim_end_matches('\n');
        if content.trim() == marker {
            return Some((offset, offset + content.trim_end().len()));
        }
        offset += line.len();
    }
    None
}

/// Replace (or insert) a delimited block (`begin_marker`..`end_marker`) inside `crontab` text.
pub(crate) fn replace_block_with(
    crontab: &str,
    block: &str,
    begin_marker: &str,
    end_marker: &str,
) -> String {
    let begin_pos = find_marker_line(crontab, begin_marker).map(|(start, _)| start);
    let end_pos = find_marker_line(crontab, end_marker).map(|(_, marker_end)| marker_end);

    match (begin_pos, end_pos) {
        (Some(begin), Some(end)) if begin < end => {
            let after = end;
            let mut result = crontab[..begin].to_string();
            result.push_str(block);
            result.push('\n');
            let rest = crontab[after..].trim_start_matches('\n');
            if !rest.is_empty() {
                result.push('\n');
                result.push_str(rest);
                // Preserve trailing newline from original if present.
                if !result.ends_with('\n') {
                    result.push('\n');
                }
            }
            result
        }
        (Some(begin), _) => {
            // Malformed block (begin without end): replace from begin to end of string.
            let mut result = crontab[..begin].to_string();
            result.push_str(block);
            result.push('\n');
            result
        }
        _ => {
            // No existing block — append after existing content.
            let mut result = crontab.trim_end_matches('\n').to_string();
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(block);
            result.push('\n');
            result
        }
    }
}

// ─── Public sync API ───────────────────────────────────────────────────────

/// Remove the managed routines crontab block (`# BEGIN MOADIM-ROUTINES`) from the user's
/// crontab, leaving every other entry untouched. Returns the number of managed schedule
/// lines removed.
///
/// Used by `moadim uninstall`: install registers an OS service *and* sync writes this
/// crontab block, so a clean teardown must clear it — otherwise `cron` keeps firing
/// routines against a daemon the user removed.
///
/// Best-effort and idempotent: a crontab with no managed block (or no crontab at all)
/// removes nothing and returns `0` without rewriting the crontab.
#[cfg(not(test))]
pub(crate) const fn clear_managed_crontab_blocks() -> Result<usize, SyncError> {
    Ok(0)
}

/// Clear managed routine entries from the OS crontab.
#[cfg(test)]
pub(crate) fn clear_managed_crontab_blocks() -> Result<usize, SyncError> {
    let current = read_crontab()?;

    // Count managed schedule lines before removal for the user-facing report.
    let removed = current
        .lines()
        .filter(|line| line.contains(routines::ROUTINE_LINE_MARKER))
        .count();

    if !current.contains(routines::BLOCK_BEGIN) {
        return Ok(0);
    }
    // No `updated == current` idempotency check here (contrast `sync_to_crontab`/
    // `sync_routines_to_crontab`): given the `BLOCK_BEGIN` guard above, `replace_block_with`
    // always strips at least the begin marker from `current`, so `updated` can never equal
    // `current` — the guard above is what makes repeated calls idempotent (a second call sees
    // no `BLOCK_BEGIN` and returns `Ok(0)` before reaching this point).
    let updated = replace_block_with(&current, "", routines::BLOCK_BEGIN, routines::BLOCK_END);
    write_crontab(&updated)?;
    Ok(removed)
}
