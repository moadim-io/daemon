//! OS-specific scheduler and launcher backends.
//!
//! The rest of the daemon is platform-neutral; the genuinely OS-divergent pieces live here. On Unix
//! managed schedules are crontab blocks and agents run in tmux (implemented in [`crate::sync`] and
//! [`crate::routines`]); on Windows they are Task Scheduler tasks driving PowerShell `run.ps1`
//! scripts ([`windows`]). The cron→`schtasks` schedule translation ([`schedule`]) is pure and is
//! also compiled under test so it can be exercised on any host.

#[cfg(any(windows, test))]
pub mod schedule;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;
