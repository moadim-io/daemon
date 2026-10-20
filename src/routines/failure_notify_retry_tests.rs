//! Webhook retry cases split out of `failure_notify_dispatch_tests.rs` (line budget).

use super::*;

/// A `curl` shim that fails on its first `fail_count` invocations (writing
/// nothing) then succeeds and writes `stdin` to the final positional arg.
/// Tracks the number of invocations in `dir/attempts.count`.
fn flaky_curl_shim(dir: &std::path::Path, fail_count: u32) -> std::path::PathBuf {
    let path = dir.join("curl-flaky.sh");
    let counter = dir.join("attempts.count");
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\nn=$(cat {counter} 2>/dev/null || echo 0)\nn=$((n+1))\necho \"$n\" > {counter}\nfor arg do target=\"$arg\"; done\nif [ \"$n\" -le {fail_count} ]; then\n  printf err >&2\n  exit 1\nfi\ncat > \"$target\"\nexit 0\n",
            counter = counter.display(),
        ),
    )
    .unwrap();
    std::process::Command::new("chmod")
        .arg("+x")
        .arg(&path)
        .status()
        .unwrap();
    path
}

#[test]
fn post_webhook_retries_and_delivers_after_transient_failures() {
    let dir = temp_dir("webhook-retry-success");
    let sink = dir.join("sink.json");
    // Fails on every attempt but the last, so delivery only succeeds because
    // of the retry loop.
    let shim = flaky_curl_shim(&dir, WEBHOOK_MAX_ATTEMPTS - 1);

    post_webhook(
        &shim.to_string_lossy(),
        &sink.to_string_lossy(),
        b"{\"ok\":true}",
    );

    assert_eq!(
        std::fs::read_to_string(&sink).unwrap(),
        "{\"ok\":true}",
        "final attempt should have delivered the payload"
    );
    let attempts: u32 = std::fs::read_to_string(dir.join("attempts.count"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(attempts, WEBHOOK_MAX_ATTEMPTS);
}

#[test]
fn post_webhook_gives_up_and_logs_after_exhausting_retries() {
    let dir = temp_dir("webhook-retry-exhausted");
    let sink = dir.join("sink.json");
    // Always fails: even the last attempt is a failure.
    let shim = flaky_curl_shim(&dir, WEBHOOK_MAX_ATTEMPTS + 5);

    post_webhook(&shim.to_string_lossy(), &sink.to_string_lossy(), b"{}");

    assert!(!sink.exists(), "payload should never have been delivered");
    let attempts: u32 = std::fs::read_to_string(dir.join("attempts.count"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(
        attempts, WEBHOOK_MAX_ATTEMPTS,
        "should stop retrying at the bounded attempt limit"
    );
}
