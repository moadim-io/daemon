//! Live auto-refresh control for the data tables.
//!
//! Renders a Grafana/Datadog-style refresh-interval selector plus an "updated
//! Ns ago" freshness cue, letting an operator keep the routines list current on
//! a cadence they choose (Off / 5s / 15s / 30s / 60s) instead of reloading the
//! SPA. The chosen interval is persisted to `localStorage`, so it is consistent
//! across pages and survives navigation and reload.
//!
//! The interval codec and freshness formatting are pure and host-tested (see
//! `refresh_tests.rs`); only the `RefreshControl` component, its 1s display
//! tick, and the `localStorage` round-trip touch the DOM/wasm layer.

use gloo_timers::future::TimeoutFuture;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlSelectElement;
use yew::prelude::*;

/// `localStorage` key the selected interval persists under. Shared so the
/// choice is fleet-wide and consistent across pages.
const STORAGE_KEY: &str = "moadim.refresh-interval";

/// Operator-selectable auto-refresh cadence for a data table. `Off` (the
/// default) preserves the historical load-once behaviour — no background
/// traffic until the operator opts in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RefreshInterval {
    /// No auto-refresh; the list loads once on mount.
    #[default]
    Off,
    /// Refresh every 5 seconds.
    S5,
    /// Refresh every 15 seconds.
    S15,
    /// Refresh every 30 seconds.
    S30,
    /// Refresh every 60 seconds.
    S60,
}

impl RefreshInterval {
    /// Every variant in selector order, for rendering the dropdown.
    pub const ALL: [Self; 5] = [Self::Off, Self::S5, Self::S15, Self::S30, Self::S60];

    /// The cadence in milliseconds, or `None` for `Off` (no auto-refresh).
    pub fn as_millis(self) -> Option<u32> {
        match self {
            Self::Off => None,
            Self::S5 => Some(5_000),
            Self::S15 => Some(15_000),
            Self::S30 => Some(30_000),
            Self::S60 => Some(60_000),
        }
    }

    /// Short human label shown in the dropdown.
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::S5 => "5s",
            Self::S15 => "15s",
            Self::S30 => "30s",
            Self::S60 => "60s",
        }
    }

    /// Stable token used as the `<option>` value and the persisted form.
    pub fn to_token(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::S5 => "5",
            Self::S15 => "15",
            Self::S30 => "30",
            Self::S60 => "60",
        }
    }

    /// Parse a token back into an interval, defaulting to `Off` for anything
    /// unrecognized (older/garbage `localStorage` values fall back safely).
    pub fn from_token(token: &str) -> Self {
        match token {
            "5" => Self::S5,
            "15" => Self::S15,
            "30" => Self::S30,
            "60" => Self::S60,
            _ => Self::Off,
        }
    }
}

/// Format "time since last load" for the freshness cue: "updated just now"
/// under a minute, then "updated Nm ago" / "updated Nh ago".
pub fn fmt_freshness(secs_ago: u64) -> String {
    if secs_ago < 60 {
        "updated just now".into()
    } else if secs_ago < 3_600 {
        format!("updated {}m ago", secs_ago / 60)
    } else {
        format!("updated {}h ago", secs_ago / 3_600)
    }
}

/// Read the persisted interval from `localStorage`, defaulting to `Off` when
/// storage is unavailable or holds no/garbage value.
pub fn load_interval() -> RefreshInterval {
    let token = web_sys::window()
        .and_then(|win| win.local_storage().ok().flatten())
        .and_then(|store| store.get_item(STORAGE_KEY).ok().flatten());
    match token {
        Some(token) => RefreshInterval::from_token(&token),
        None => RefreshInterval::Off,
    }
}

/// Persist the chosen interval to `localStorage`. Best-effort: a storage error
/// (e.g. private-mode quota) is silently ignored — the in-memory choice still
/// applies for the session.
pub fn save_interval(interval: RefreshInterval) {
    if let Some(store) = web_sys::window().and_then(|win| win.local_storage().ok().flatten()) {
        let _ = store.set_item(STORAGE_KEY, interval.to_token());
    }
}

#[derive(Properties, PartialEq)]
pub struct RefreshControlProps {
    /// Currently selected interval.
    pub interval: RefreshInterval,
    /// `Date.now()` (ms) of the last successful list load; `0.0` means the list
    /// has not loaded yet, which hides the freshness cue.
    pub updated_at_ms: f64,
    /// Emitted with the newly chosen interval when the operator changes it.
    pub on_change: Callback<RefreshInterval>,
}

/// Interval dropdown + live freshness label for a data table's action row.
#[function_component(RefreshControl)]
pub fn refresh_control(props: &RefreshControlProps) -> Html {
    // A local 1s tick re-renders just this widget so the "updated Ns ago" label
    // stays live, without forcing the parent table to re-render every second.
    // The stored instant is only a re-render trigger, so each tick stamps a
    // fresh value; the label is recomputed from `updated_at_ms` below.
    let tick = use_state(|| 0.0_f64);
    {
        use_effect_with((), move |()| {
            spawn_local(async move {
                loop {
                    TimeoutFuture::new(1_000).await;
                    tick.set(js_sys::Date::now());
                }
            });
        });
    }

    let on_select = {
        let on_change = props.on_change.clone();
        Callback::from(move |event: Event| {
            let select: HtmlSelectElement = event.target_unchecked_into();
            on_change.emit(RefreshInterval::from_token(&select.value()));
        })
    };

    let current = props.interval;
    let freshness = if props.updated_at_ms > 0.0 {
        let secs = ((js_sys::Date::now() - props.updated_at_ms).max(0.0) / 1000.0) as u64;
        Some(fmt_freshness(secs))
    } else {
        None
    };

    html! {
        <div class="refresh-control">
            <label class="refresh-lbl" for="refresh-interval">{"AUTO"}</label>
            <select
                id="refresh-interval"
                class="refresh-select"
                onchange={on_select}
                aria-label="Auto-refresh interval"
            >
                { for RefreshInterval::ALL.iter().map(|interval| html! {
                    <option value={interval.to_token()} selected={*interval == current}>
                        { interval.label() }
                    </option>
                }) }
            </select>
            {
                match freshness {
                    Some(text) => html! {
                        <span class="refresh-fresh" title="Time since the list last refreshed">
                            { text }
                        </span>
                    },
                    None => html! {},
                }
            }
        </div>
    }
}

#[cfg(test)]
#[path = "refresh_tests.rs"]
mod refresh_tests;
