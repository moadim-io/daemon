#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use std::sync::Mutex;

use super::current_user;

static ENV_GUARD: Mutex<()> = Mutex::new(());

fn with_user_env(user: Option<&str>, username: Option<&str>, body: impl FnOnce()) {
    let guard = ENV_GUARD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let saved_user = std::env::var_os("USER");
    let saved_username = std::env::var_os("USERNAME");
    // SAFETY: this test serializes env mutations with ENV_GUARD.
    unsafe {
        match user {
            Some(value) => std::env::set_var("USER", value),
            None => std::env::remove_var("USER"),
        }
        match username {
            Some(value) => std::env::set_var("USERNAME", value),
            None => std::env::remove_var("USERNAME"),
        }
    }
    body();
    // SAFETY: this test serializes env mutations with ENV_GUARD.
    unsafe {
        match saved_user {
            Some(value) => std::env::set_var("USER", value),
            None => std::env::remove_var("USER"),
        }
        match saved_username {
            Some(value) => std::env::set_var("USERNAME", value),
            None => std::env::remove_var("USERNAME"),
        }
    }
    drop(guard);
}

#[test]
fn current_user_prefers_user_env_when_present() {
    with_user_env(Some("alice"), Some("bob"), || {
        assert_eq!(current_user().as_deref(), Some("alice"));
    });
}

#[test]
fn current_user_falls_back_to_username_and_ignores_blank_values() {
    with_user_env(None, Some("bob"), || {
        assert_eq!(current_user().as_deref(), Some("bob"));
    });
    with_user_env(Some("   "), None, || {
        assert_eq!(current_user(), None);
    });
}
