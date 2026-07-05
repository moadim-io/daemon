//! Routines tab: list, create, edit, trigger, logs, and delete agent-driven scheduled jobs.
//!
//! Targets the `/routines` API. A routine launches an AI agent (claude, codex, …) on a
//! schedule.
//!
//! Split by concern, mirroring `src/routines/`:
//! - `model` — data types + the `/routines` API client.
//! - `filter` — faceted filter + derived operational health.
//! - `state` — page/view/modal state, group-by, sorting, and the reducer.
//! - `hooks` — small custom Yew hooks used by the list page.
//! - `page` — the `RoutinesPage` component that wires the above together.
//! - `banner`, `filter_bar`, `calendar`, `table`, `row`, `form`, `bulk`, `logs`,
//!   `flags_panel` — the list page's sub-components.

mod banner;
mod bulk;
mod calendar;
mod filter;
mod filter_bar;
mod flags_panel;
mod form;
mod history;
mod hooks;
mod logs;
mod model;
mod page;
mod row;
mod state;
mod table;

// Only re-export what callers outside this module tree actually reach for
// (`crate::routines::{Routine, api_unlock, GlobalLockBanner, LockStatus,
// RoutinesPage}`); every other cross-submodule reference goes through an
// explicit `use super::x::Y;` in the submodule that needs it.
pub use banner::GlobalLockBanner;
pub(crate) use history::{run_status_class, run_status_label};
pub use model::*;
pub use page::RoutinesPage;
pub(crate) use state::RoutineHistoryQuery;
