use croner::Cron;
use gloo_timers::future::TimeoutFuture;
use serde::Deserialize;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

mod command_palette;
mod day_timeline;
mod log_viewer;
mod machines;
mod overview;
mod overview_recent_runs;
mod overview_upcoming;
mod refresh;
mod routines;
mod schedule;
mod schedule_heatmap;
mod settings;
mod shell_dialogs;
use command_palette::CommandPalette;
use overview::OverviewPage;
use routines::RoutinesPage;
use schedule_heatmap::HeatmapPage;
use settings::SettingsPage;
use shell_dialogs::{
    api_get_machine, api_put_machine, api_shutdown, fmt_uptime, poll_health, RenameMachineDialog,
    ShutdownDialog, ToastStack,
};

// ─── Theme ────────────────────────────────────────────────────────────────────

/// localStorage key for the theme preference.
pub(crate) const THEME_KEY: &str = "moadim.theme";

/// Read the persisted theme from localStorage. Returns `true` for light theme.
pub(crate) fn load_theme_light() -> bool {
    web_sys::window()
        .and_then(|win| win.local_storage().ok().flatten())
        .and_then(|store| store.get_item(THEME_KEY).ok().flatten())
        .is_some_and(|val| val == "light")
}

/// Persist the theme choice to localStorage (best-effort; ignores storage errors).
pub(crate) fn save_theme_light(light: bool) {
    if let Some(store) = web_sys::window().and_then(|win| win.local_storage().ok().flatten()) {
        let _ = store.set_item(THEME_KEY, if light { "light" } else { "dark" });
    }
}

/// Apply or remove the `theme-light` CSS class from `<html>`.
pub(crate) fn apply_theme(light: bool) {
    if let Some(root) = web_sys::window()
        .and_then(|win| win.document())
        .and_then(|doc| doc.document_element())
    {
        let list = root.class_list();
        if light {
            let _ = list.add_1("theme-light");
        } else {
            let _ = list.remove_1("theme-light");
        }
    }
}

// ─── Shared types ─────────────────────────────────────────────────────────────

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

// ─── Routes ───────────────────────────────────────────────────────────────────

/// Top-level pages, served at the root path space: `Routines` at `/routines` and `Heatmap` at
/// `/heatmap`. The REST API is namespaced under `/api/v1`, so these UI paths never collide with it.
/// The server returns the same self-contained HTML for any unmatched path (SPA fallback), letting
/// these deep links and refreshes load the app so the router can resolve the path.
#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/routines")]
    Routines,
    #[at("/heatmap")]
    Heatmap,
    #[at("/settings")]
    Settings,
    #[not_found]
    #[at("/404")]
    NotFound,
}

// ─── Shell state (health, toasts, shutdown) ───────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShellState {
    pub health: Health,
    pub health_ok: bool,
    pub toasts: Vec<Toast>,
    pub next_toast: u32,
    pub show_shutdown: bool,
    pub show_palette: bool,
    /// `true` when the light theme is active; persisted to localStorage.
    pub show_theme_light: bool,
    /// Resolved name of this machine, fetched from `GET /api/v1/machine` on mount.
    pub machine_name: Option<String>,
    /// Whether the rename-machine dialog is open.
    pub show_rename_machine: bool,
}

pub enum ShellAction {
    HealthLoaded { health: Health, ok: bool },
    AddToast { msg: String, kind: ToastKind },
    OpenShutdown,
    CloseShutdown,
    TogglePalette,
    ClosePalette,
    ToggleTheme,
    MachineName { name: String },
    OpenRenameMachine,
    CloseRenameMachine,
}

impl Reducible for ShellState {
    type Action = ShellAction;

    fn reduce(self: std::rc::Rc<Self>, action: Self::Action) -> std::rc::Rc<Self> {
        let mut s = (*self).clone();
        match action {
            ShellAction::HealthLoaded { health, ok } => {
                s.health = health;
                s.health_ok = ok;
            }
            ShellAction::AddToast { msg, kind } => {
                let id = s.next_toast;
                s.next_toast += 1;
                s.toasts.push(Toast {
                    id,
                    msg: AttrValue::from(msg),
                    kind,
                });
            }
            ShellAction::OpenShutdown => s.show_shutdown = true,
            ShellAction::CloseShutdown => s.show_shutdown = false,
            ShellAction::TogglePalette => s.show_palette = !s.show_palette,
            ShellAction::ClosePalette => s.show_palette = false,
            ShellAction::ToggleTheme => {
                s.show_theme_light = !s.show_theme_light;
                save_theme_light(s.show_theme_light);
                apply_theme(s.show_theme_light);
            }
            ShellAction::MachineName { name } => s.machine_name = Some(name),
            ShellAction::OpenRenameMachine => s.show_rename_machine = true,
            ShellAction::CloseRenameMachine => s.show_rename_machine = false,
        }
        s.into()
    }
}

// ─── Root ─────────────────────────────────────────────────────────────────────

#[function_component(App)]
pub fn app() -> Html {
    html! {
        <BrowserRouter>
            <Shell />
        </BrowserRouter>
    }
}

/// Persistent chrome around the routed pages: header (health + STOP), nav tabs, the routed page,
/// the global shutdown dialog, and the toast stack. Lives inside the router so nav `Link`s work.
#[function_component(Shell)]
pub fn shell() -> Html {
    let state = use_reducer(|| ShellState {
        show_theme_light: load_theme_light(),
        ..ShellState::default()
    });

    // Apply the initial theme class from persisted preference.
    {
        let light = state.show_theme_light;
        use_effect_with((), move |_| {
            apply_theme(light);
        });
    }

    // Initial health poll + machine name fetch on mount.
    {
        let state = state.clone();
        use_effect_with((), move |_| {
            let state2 = state.clone();
            spawn_local(async move { poll_health(state).await });
            spawn_local(async move {
                if let Ok(name) = api_get_machine().await {
                    state2.dispatch(ShellAction::MachineName { name });
                }
            });
        });
    }

    // Health poll loop every 30 s.
    {
        let state = state.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                loop {
                    TimeoutFuture::new(30_000).await;
                    poll_health(state.clone()).await;
                }
            });
        });
    }

    // Global ⌘K / Ctrl-K listener that toggles the command palette from any
    // page, and Escape to dismiss whichever shell-level dialog is open.
    // Registered once on mount and torn down on unmount.
    {
        let state = state.clone();
        use_effect_with((), move |_| {
            let on_key =
                Closure::<dyn Fn(KeyboardEvent)>::wrap(Box::new(move |event: KeyboardEvent| {
                    if (event.meta_key() || event.ctrl_key())
                        && event.key().eq_ignore_ascii_case("k")
                    {
                        event.prevent_default();
                        state.dispatch(ShellAction::TogglePalette);
                    } else if event.key() == "Escape" {
                        if state.show_shutdown {
                            state.dispatch(ShellAction::CloseShutdown);
                        } else if state.show_rename_machine {
                            state.dispatch(ShellAction::CloseRenameMachine);
                        }
                    }
                }));
            let window = web_sys::window().expect("window exists");
            window
                .add_event_listener_with_callback("keydown", on_key.as_ref().unchecked_ref())
                .expect("keydown listener attaches");
            move || {
                if let Some(window) = web_sys::window() {
                    let _ = window.remove_event_listener_with_callback(
                        "keydown",
                        on_key.as_ref().unchecked_ref(),
                    );
                }
                drop(on_key);
            }
        });
    }

    let on_toast = {
        let state = state.clone();
        Callback::from(move |(msg, kind): (String, ToastKind)| {
            state.dispatch(ShellAction::AddToast { msg, kind })
        })
    };

    let on_close_palette = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(ShellAction::ClosePalette))
    };

    let on_open_palette = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| state.dispatch(ShellAction::TogglePalette))
    };

    // Palette "Refresh" / "Stop Server" actions mirror the header buttons but
    // take the `()` payload the palette emits.
    let on_palette_refresh = {
        let state = state.clone();
        Callback::from(move |_: ()| {
            let state = state.clone();
            spawn_local(async move { poll_health(state).await });
        })
    };
    let on_palette_stop = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(ShellAction::OpenShutdown))
    };
    let on_palette_toggle_theme = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(ShellAction::ToggleTheme))
    };

    let on_refresh = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| {
            let state = state.clone();
            spawn_local(async move { poll_health(state).await });
        })
    };

    // STOP only opens a confirmation dialog; the server is asked to stop on confirm.
    let on_stop = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| state.dispatch(ShellAction::OpenShutdown))
    };

    let on_close_shutdown = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(ShellAction::CloseShutdown))
    };

    let on_confirm_shutdown = {
        let state = state.clone();
        Callback::from(move |_: ()| {
            let state = state.clone();
            state.dispatch(ShellAction::CloseShutdown);
            spawn_local(async move {
                match api_shutdown().await {
                    Ok(()) => {
                        state.dispatch(ShellAction::HealthLoaded {
                            health: Health {
                                status: "stopping".into(),
                                running: false,
                                uptime_secs: None,
                                version: None,
                                git_sha: None,
                                dependencies: None,
                            },
                            ok: false,
                        });
                        state.dispatch(ShellAction::AddToast {
                            msg: "Server stopping…".into(),
                            kind: ToastKind::Ok,
                        });
                    }
                    Err(e) => state.dispatch(ShellAction::AddToast {
                        msg: format!("Stop failed: {e}"),
                        kind: ToastKind::Err,
                    }),
                }
            });
        })
    };

    let switch = {
        let on_toast = on_toast.clone();
        Callback::from(move |route: Route| match route {
            Route::Home => html! { <OverviewPage on_toast={on_toast.clone()} /> },
            Route::Routines => html! { <RoutinesPage on_toast={on_toast.clone()} /> },
            Route::Heatmap => html! { <HeatmapPage /> },
            Route::Settings => html! { <SettingsPage on_toast={on_toast.clone()} /> },
            Route::NotFound => html! { <Redirect<Route> to={Route::Home} /> },
        })
    };

    let health = state.health.clone();
    let health_ok = state.health_ok;
    let toasts = state.toasts.clone();
    let show_shutdown = state.show_shutdown;
    let show_palette = state.show_palette;
    let show_theme_light = state.show_theme_light;
    let machine_name = state.machine_name.clone();
    let show_rename_machine = state.show_rename_machine;
    let on_theme = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| state.dispatch(ShellAction::ToggleTheme))
    };

    let on_open_rename_machine = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| state.dispatch(ShellAction::OpenRenameMachine))
    };
    let on_close_rename_machine = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(ShellAction::CloseRenameMachine))
    };
    let on_confirm_rename_machine = {
        let state = state.clone();
        Callback::from(
            move |(name, on_done): (String, Callback<Result<(), String>>)| {
                let state = state.clone();
                spawn_local(async move {
                    match api_put_machine(&name).await {
                        Ok(new_name) => {
                            state.dispatch(ShellAction::MachineName {
                                name: new_name.clone(),
                            });
                            state.dispatch(ShellAction::CloseRenameMachine);
                            state.dispatch(ShellAction::AddToast {
                                msg: format!("Machine renamed to \"{new_name}\""),
                                kind: ToastKind::Ok,
                            });
                            on_done.emit(Ok(()));
                        }
                        Err(e) => {
                            state.dispatch(ShellAction::AddToast {
                                msg: format!("Rename failed: {e}"),
                                kind: ToastKind::Err,
                            });
                            on_done.emit(Err(e));
                        }
                    }
                });
            },
        )
    };

    html! {
        <>
            <Header health={health} ok={health_ok} light={show_theme_light}
                machine_name={machine_name.clone()}
                on_refresh={on_refresh} on_stop={on_stop} on_palette={on_open_palette}
                on_theme={on_theme} on_rename_machine={on_open_rename_machine} />
            <Nav />
            <Switch<Route> render={switch} />
            <CommandPalette
                open={show_palette}
                on_close={on_close_palette}
                on_refresh={on_palette_refresh}
                on_stop={on_palette_stop}
                on_toggle_theme={on_palette_toggle_theme}
            />
            {
                if show_rename_machine {
                    html! {
                        <RenameMachineDialog
                            current={machine_name.unwrap_or_default()}
                            on_cancel={on_close_rename_machine}
                            on_confirm={on_confirm_rename_machine}
                        />
                    }
                } else {
                    html! {}
                }
            }
            {
                if show_shutdown {
                    html! {
                        <ShutdownDialog
                            on_cancel={on_close_shutdown}
                            on_confirm={on_confirm_shutdown}
                        />
                    }
                } else {
                    html! {}
                }
            }
            <ToastStack toasts={toasts} />
        </>
    }
}

// ─── Nav ──────────────────────────────────────────────────────────────────────

#[function_component(Nav)]
pub fn nav() -> Html {
    let route = use_route::<Route>().unwrap_or(Route::Home);
    let cls = |target: &Route| {
        if route == *target {
            "tab-btn active"
        } else {
            "tab-btn"
        }
    };
    html! {
        <nav class="tabs">
            <Link<Route> classes={classes!(cls(&Route::Home))} to={Route::Home}>
                { "OVERVIEW" }
            </Link<Route>>
            <Link<Route> classes={classes!(cls(&Route::Routines))} to={Route::Routines}>
                { "ROUTINES" }
            </Link<Route>>
            <Link<Route> classes={classes!(cls(&Route::Heatmap))} to={Route::Heatmap}>
                { "HEATMAP" }
            </Link<Route>>
            <Link<Route> classes={classes!(cls(&Route::Settings))} to={Route::Settings}>
                { "SETTINGS" }
            </Link<Route>>
        </nav>
    }
}

// ─── Header ───────────────────────────────────────────────────────────────────

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
                if !version_title.is_empty() {
                    <span class="logo-version" title={version_title}>{version_text}</span>
                } else {
                    <span class="logo-version">{version_text}</span>
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

// ─── Utilities (shared with the routines module) ───────────────────────────────

/// Parse a cron expression into a `Cron`, normalizing the 7-field
/// (sec min hour dom month dow year) form to 5-field to match server behaviour.
/// Returns `None` for empty or invalid expressions.
pub(crate) fn parse_cron(expr: &str) -> Option<Cron> {
    let s = expr.trim();
    if s.is_empty() {
        return None;
    }
    let normalized = if s.starts_with('@') {
        s.to_string()
    } else {
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() == 7 {
            parts[1..6].join(" ")
        } else {
            s.to_string()
        }
    };
    normalized.parse::<Cron>().ok()
}

/// Returns (is_valid, human description) for a cron expression.
pub(crate) fn describe_cron_live(expr: &str) -> (bool, String) {
    if expr.trim().is_empty() {
        return (false, "— enter a cron expression —".into());
    }
    match parse_cron(expr) {
        Some(cron) => (true, cron.describe()),
        None => (false, "Invalid cron expression".into()),
    }
}

pub(crate) fn reltime(ts: u64) -> String {
    if ts == 0 {
        return "—".into();
    }
    let now = (js_sys::Date::now() / 1000.0) as u64;
    let diff = now.saturating_sub(ts);
    if diff < 60 {
        "just now".into()
    } else if diff < 3_600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86_400 {
        format!("{}h ago", diff / 3_600)
    } else {
        format!("{}d ago", diff / 86_400)
    }
}

fn main() {
    console_log::init_with_level(log::Level::Info).unwrap_or_default();
    yew::Renderer::<App>::new().render();
}
