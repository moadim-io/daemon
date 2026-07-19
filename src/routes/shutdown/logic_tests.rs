#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::build;

#[tokio::test]
async fn build_fires_the_signal_and_acknowledges() {
    let signal = std::sync::Arc::new(tokio::sync::Notify::new());
    let notified = signal.notified();
    let response = build(&signal);
    assert_eq!(response.status, "shutting down");
    // notify_one() must have fired before build() returned, or this would hang.
    notified.await;
}
