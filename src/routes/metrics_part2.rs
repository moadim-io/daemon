
/// Render `snapshot` as Prometheus text exposition format (one `# HELP`/`# TYPE` pair per
/// series, then its sample line(s)).
fn render(snapshot: &MetricsSnapshot<'_>) -> String {
    let mut out = String::new();

    let _ = writeln!(
        out,
        "# HELP moadim_uptime_seconds Seconds since the daemon started."
    );
    let _ = writeln!(out, "# TYPE moadim_uptime_seconds gauge");
    let _ = writeln!(out, "moadim_uptime_seconds {}", snapshot.uptime_secs);

    let _ = writeln!(
        out,
        "# HELP moadim_build_info Daemon build metadata; the sample value is always 1."
    );
    let _ = writeln!(out, "# TYPE moadim_build_info gauge");
    let _ = writeln!(
        out,
        r#"moadim_build_info{{version="{}",git_sha="{}",machine="{}"}} 1"#,
        snapshot.version,
        snapshot.git_sha,
        escape_label_value(snapshot.machine)
    );

    let _ = writeln!(
        out,
        "# HELP moadim_active_sessions Number of live tmux agent sessions right now."
    );
    let _ = writeln!(out, "# TYPE moadim_active_sessions gauge");
    let _ = writeln!(out, "moadim_active_sessions {}", snapshot.active_sessions);

    let _ = writeln!(
        out,
        "# HELP moadim_workbench_bytes Total size in bytes of the workbench tree on disk."
    );
    let _ = writeln!(out, "# TYPE moadim_workbench_bytes gauge");
    let _ = writeln!(out, "moadim_workbench_bytes {}", snapshot.workbench_bytes);

    let _ = writeln!(
        out,
        "# HELP moadim_repo_cache_bytes Total size in bytes of the repository mirror cache tree ({{config_dir}}/cache/) on disk."
    );
    let _ = writeln!(out, "# TYPE moadim_repo_cache_bytes gauge");
    let _ = writeln!(out, "moadim_repo_cache_bytes {}", snapshot.repo_cache_bytes);

    render_runs_total(&mut out, snapshot.runs);
    render_run_duration_histogram(&mut out, snapshot.runs);

    let _ = writeln!(
        out,
        "# HELP moadim_cleanup_removed_total Workbenches removed by cleanup sweeps since the daemon started."
    );
    let _ = writeln!(out, "# TYPE moadim_cleanup_removed_total counter");
    let _ = writeln!(
        out,
        "moadim_cleanup_removed_total {}",
        snapshot.cleanup_removed_total
    );

    let _ = writeln!(
        out,
        "# HELP moadim_cleanup_freed_bytes_total Bytes freed by cleanup sweeps since the daemon started."
    );
    let _ = writeln!(out, "# TYPE moadim_cleanup_freed_bytes_total counter");
    let _ = writeln!(
        out,
        "moadim_cleanup_freed_bytes_total {}",
        snapshot.cleanup_freed_bytes_total
    );

    out
}

/// Append the `moadim_runs_total{status=...}` counter series, one sample per [`RunStatus`].
fn render_runs_total(out: &mut String, runs: &[FleetRunSummary]) {
    let mut counts = RunStatusCounts::default();
    for run in runs {
        match run.status {
            RunStatus::Success => counts.success += 1,
            RunStatus::Failed => counts.failed += 1,
            RunStatus::Running => counts.running += 1,
            RunStatus::Unknown => counts.unknown += 1,
        }
    }
    let _ = writeln!(
        out,
        "# HELP moadim_runs_total Total routine runs observed, by outcome."
    );
    let _ = writeln!(out, "# TYPE moadim_runs_total counter");
    let _ = writeln!(
        out,
        r#"moadim_runs_total{{status="success"}} {}"#,
        counts.success
    );
    let _ = writeln!(
        out,
        r#"moadim_runs_total{{status="failed"}} {}"#,
        counts.failed
    );
    let _ = writeln!(
        out,
        r#"moadim_runs_total{{status="running"}} {}"#,
        counts.running
    );
    let _ = writeln!(
        out,
        r#"moadim_runs_total{{status="unknown"}} {}"#,
        counts.unknown
    );
}

/// Append the `moadim_run_duration_seconds` histogram, over every run with a recorded
/// `finished_at` (i.e. every non-[`RunStatus::Running`] run, regardless of outcome).
fn render_run_duration_histogram(out: &mut String, runs: &[FleetRunSummary]) {
    let durations: Vec<u64> = runs
        .iter()
        .filter_map(|run| {
            run.finished_at
                .map(|finished| finished.saturating_sub(run.started_at))
        })
        .collect();

    let _ = writeln!(
        out,
        "# HELP moadim_run_duration_seconds Duration of finished routine runs, in seconds."
    );
    let _ = writeln!(out, "# TYPE moadim_run_duration_seconds histogram");
    // Each bucket is the count of *every* duration <= its bound (Prometheus histogram buckets
    // are cumulative from `le="+Inf"` downward, not per-range), recomputed fresh rather than
    // accumulated across the loop — with only a handful of runs per scrape, a second `filter`
    // pass per bucket is cheaper to get right than threading a running total through.
    for bound in DURATION_BUCKETS_SECS {
        let cumulative = durations.iter().filter(|&&secs| secs <= bound).count();
        let _ = writeln!(
            out,
            r#"moadim_run_duration_seconds_bucket{{le="{bound}"}} {cumulative}"#
        );
    }
    let _ = writeln!(
        out,
        r#"moadim_run_duration_seconds_bucket{{le="+Inf"}} {}"#,
        durations.len()
    );
    let sum: u64 = durations.iter().sum();
    let _ = writeln!(out, "moadim_run_duration_seconds_sum {sum}");
    let _ = writeln!(out, "moadim_run_duration_seconds_count {}", durations.len());
}

#[cfg(test)]
#[path = "metrics_tests.rs"]
mod metrics_tests;
