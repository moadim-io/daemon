//! Shared health/toast types used across the shell, header, and shutdown
//! dialog. Split out of `main.rs` to keep that file under the line-count gate;
//! re-exported from the crate root so existing `crate::Health`-style paths
//! keep working unchanged.

use serde::Deserialize;
use yew::AttrValue;

#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
pub struct HealthDeps {
    pub tmux: bool,
    pub python3: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
pub struct Health {
    pub status: String,
    pub uptime_secs: Option<u64>,
    pub running: bool,
    pub version: Option<String>,
    #[serde(default)]
    pub git_sha: Option<String>,
    #[serde(default)]
    pub dependencies: Option<HealthDeps>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToastKind {
    Ok,
    Err,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Toast {
    pub id: u32,
    pub msg: AttrValue,
    pub kind: ToastKind,
}
