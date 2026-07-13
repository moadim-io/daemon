//! The app shell's top header bar: logo, dependency warnings, health status,
//! machine badge, and the theme/palette/refresh/stop action buttons. Split out
//! of `main.rs` to keep that file under the line-count gate; used only by
//! `Shell`.

use yew::prelude::*;

use crate::shell_dialogs::fmt_uptime;
use crate::Health;

#[derive(Properties, PartialEq)]
pub struct HeaderProps {
    pub health: Health,
    pub ok: bool,
    /// `true` when the light theme is active (controls the toggle button icon).
    pub light: bool,
    /// Resolved machine name, shown as a clickable badge.
    pub machine_name: Option<String>,
    pub on_refresh: Callback<MouseEvent>,
    pub on_stop: Callback<MouseEvent>,
    pub on_palette: Callback<MouseEvent>,
    pub on_theme: Callback<MouseEvent>,
    /// Opens the rename-machine dialog.
    pub on_rename_machine: Callback<MouseEvent>,
}

#[function_component(Header)]
pub fn header(props: &HeaderProps) -> Html {
    let dot_class = if props.ok {
        "health-dot ok"
    } else {
        "health-dot error"
    };
    let status = props.health.status.to_uppercase();
    let version_text = props
        .health
        .version
        .as_ref()
        .map(|v| format!("/ v{v}"))
        .unwrap_or_default();
    let version_title = props
        .health
        .git_sha
        .as_deref()
        .filter(|s| *s != "unknown" && !s.is_empty())
        .map(|sha| format!("build: {sha}"))
        .unwrap_or_default();
    let uptime = props
        .health
        .uptime_secs
        .map(|s| format!("/ UP {}", fmt_uptime(s)))
        .unwrap_or_default();
    let theme_icon = if props.light { "☀" } else { "🌙" };
    let theme_title = if props.light {
        "Switch to dark mode"
    } else {
        "Switch to light mode"
    };
    let missing_tmux = props.health.dependencies.as_ref().is_some_and(|d| !d.tmux);
    let missing_python3 = props
        .health
        .dependencies
        .as_ref()
        .is_some_and(|d| !d.python3);

    html! {
        <header>
            <h1 class="logo">
                {"MOADIM"}
                <span class="logo-sub">{"/ CONTROL"}</span>
                if version_title.is_empty() {
                    <span class="logo-version">{version_text}</span>
                } else {
                    <span class="logo-version" title={version_title}>{version_text}</span>
                }
            </h1>
            <div class="header-right">
                if missing_tmux {
                    <span class="dep-warn" title="tmux is not on the daemon's PATH — all routine runs will silently fail">
                        {"⚠ NO TMUX"}
                    </span>
                }
                if missing_python3 {
                    <span class="dep-warn dep-warn-soft" title="python3 is not on the daemon's PATH — the claude agent setup step will fail silently">
                        {"⚠ NO PYTHON3"}
                    </span>
                }
                <div class="health">
                    <div class={dot_class}></div>
                    <span class="health-status">{status}</span>
                    <span class="health-uptime">{uptime}</span>
                </div>
                if let Some(name) = &props.machine_name {
                    <button class="machine-badge" title="Click to rename this machine"
                        onclick={props.on_rename_machine.clone()}>
                        {name.clone()}
                    </button>
                }
                <button class="btn-theme" title={theme_title} aria-label={theme_title} onclick={props.on_theme.clone()}>
                    {theme_icon}
                </button>
                <button class="btn-cmdk" title="Command palette (⌘K)" aria-label="Open command palette" onclick={props.on_palette.clone()}>
                    {"⌘K"}
                </button>
                <button class="btn-refresh" title="Refresh" aria-label="Refresh" onclick={props.on_refresh.clone()}>{"↻"}</button>
                <button class="btn-stop" title="Stop the server" disabled={!props.ok} onclick={props.on_stop.clone()}>{"⏻ STOP"}</button>
            </div>
        </header>
    }
}
