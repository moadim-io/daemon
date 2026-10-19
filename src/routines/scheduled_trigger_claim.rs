//! In-memory claims used to deduplicate simultaneous scheduled fires.
//!
//! Durable `scheduled.log` entries describe execution history and survive daemon restarts. They
//! must not act as scheduler claims: a new scheduler fire in the same minute can otherwise be
//! rejected solely because the daemon reloaded that history. These process-local claims instead
//! cover only competing scheduled-trigger requests handled by this daemon instance.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::utils::lock::LockRecover;

/// Return whether this daemon instance can claim `id`'s current scheduled-fire minute.
pub(super) fn claim_current_minute(id: &str, now: u64) -> bool {
    let mut claims = scheduled_trigger_claims().lock_recover();
    claims.retain(|_, claimed_at| *claimed_at / 60 == now / 60);
    if claims
        .get(id)
        .is_some_and(|claimed_at| claimed_at / 60 == now / 60)
    {
        return false;
    }
    claims.insert(id.to_string(), now);
    true
}

/// Hold the current-minute scheduled-fire claims for this daemon process.
fn scheduled_trigger_claims() -> &'static Mutex<HashMap<String, u64>> {
    static CLAIMS: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
    CLAIMS.get_or_init(Mutex::default)
}
