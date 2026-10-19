/// Bounded retry count for a dropped webhook delivery: the failure notification
/// is the one delivery that matters most, but retries must stay bounded so a
/// dead endpoint can't pile up threads across successive routine failures.
const WEBHOOK_MAX_ATTEMPTS: u32 = 3;
/// Backoff before the first retry; doubles after each subsequent failure.
const WEBHOOK_RETRY_BACKOFF: std::time::Duration = std::time::Duration::from_millis(100);

fn post_webhook(curl_bin: &str, webhook: &str, body: &[u8]) {
    let mut backoff = WEBHOOK_RETRY_BACKOFF;
    for attempt in 1..=WEBHOOK_MAX_ATTEMPTS {
        match post_webhook_once(curl_bin, webhook, body) {
            Ok(()) => return,
            Err(err) => {
                if attempt == WEBHOOK_MAX_ATTEMPTS {
                    log::warn!(
                        "failure notification webhook failed after {WEBHOOK_MAX_ATTEMPTS} attempts: {err}"
                    );
                    return;
                }
                std::thread::sleep(backoff);
                backoff *= 2;
            }
        }
    }
}

/// Runs a single `curl` attempt, returning the failure reason on error.
#[allow(clippy::expect_used, reason = "spawned curl child should wait")]
fn post_webhook_once(curl_bin: &str, webhook: &str, body: &[u8]) -> Result<(), String> {
    let mut child = std::process::Command::new(curl_bin)
        .arg("-fsS")
        .arg("--max-time")
        .arg("10")
        .arg("-X")
        .arg("POST")
        .arg("-H")
        .arg("content-type: application/json")
        .arg("--data-binary")
        .arg("@-")
        .arg(webhook)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to spawn curl: {err}"))?;
    let _ = child.stdin.take().map(|mut stdin| stdin.write_all(body));
    let out = child.wait_with_output().expect("curl child exits");
    if out.status.success() {
        return Ok(());
    }
    Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
}
