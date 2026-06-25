use croner::Cron;
use gloo_net::http::Request;
use gloo_timers::future::TimeoutFuture;
use serde::Deserialize;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

mod command_palette;
mod cron_jobs;
mod day_timeline;
mod log_viewer;
mod machines;
mod overview;
mod refresh;
mod routines;
mod schedule;
mod schedule_heatmap;
use command_palette::CommandPalette;
use cron_jobs::CronJobsPage;
use overview::OverviewPage;
use routines::RoutinesPage;
use schedule_heatmap::HeatmapPage;

// ─── Shared types ─────────────────────────────────────────────────────────────

/// Active colour palette. Defaults to dark (the `:root` definition in index.html).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    pub(crate) fn toggle(self) -> Self {
        match self {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Dark,
        }
    }
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
        }
    }
    /// Icon shown on the toggle button — indicates the mode you will switch TO.
    pub(crate) fn icon(self) -> &'static str {
        match self {
            Theme::Dark => "☀",
            Theme::Light => "◑",
        }
    }
    pub(crate) fn aria_label(self) -> &'static str {
        match self {
            Theme::Dark => "Switch to light mode",
            Theme::Light => "Switch to dark mode",
        }
    }
}

/// Parse a `localStorage` value into a [`Theme`]. Any value other than `"light"`
/// resolves to `Dark`, matching the flash-free script's default behaviour.
pub(crate) fn parse_theme(val: Option<&str>) -> Theme {
    match val {
        Some("light") => Theme::Light,
        _ => Theme::Dark,
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
pub struct Health {
    pub status: String,
    pub uptime_secs: Option<u64>,
    pub running: bool,
    pub version: Option<String>,
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

/// Top-level pages, served at the root path space: `CronJobs` at `/cron-jobs` and `Routines` at
/// `/routines`. The REST API is namespaced under `/api/v1`, so these UI paths never collide with it.
/// The server returns the same self-contained HTML for any unmatched path (SPA fallback), letting
/// these deep links and refreshes load the app so the router can resolve the path.
#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/cron-jobs")]
    CronJobs,
    #[at("/routines")]
    Routines,
    #[at("/heatmap")]
    Heatmap,
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
    pub theme: Theme,
}

pub enum ShellAction {
    HealthLoaded { health: Health, ok: bool },
    AddToast { msg: String, kind: ToastKind },
    OpenShutdown,
    CloseShutdown,
    TogglePalette,
    ClosePalette,
    SetTheme(Theme),
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
            ShellAction::SetTheme(t) => s.theme = t,
        }
        s.into()
    }
}

// ─── API layer ────────────────────────────────────────────────────────────────

async fn api_health() -> Result<Health, String> {
    Request::get("/api/v1/health")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Health>()
        .await
        .map_err(|e| e.to_string())
}

async fn api_shutdown() -> Result<(), String> {
    let resp = Request::post("/api/v1/shutdown")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        Ok(())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}

async fn poll_health(state: UseReducerHandle<ShellState>) {
    match api_health().await {
        Ok(health) => {
            let ok = health.running;
            state.dispatch(ShellAction::HealthLoaded { health, ok });
        }
        Err(_) => state.dispatch(ShellAction::HealthLoaded {
            health: Health {
                status: "offline".into(),
                running: false,
                uptime_secs: None,
                version: None,
            },
            ok: false,
        }),
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
    let state = use_reducer(ShellState::default);

    // Initial health poll on mount.
    {
        let state = state.clone();
        use_effect_with((), move |_| {
            spawn_local(async move { poll_health(state).await });
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
    // page. Registered once on mount and torn down on unmount.
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

    // Read the persisted theme from localStorage on mount and sync the reducer.
    // The flash-free <script> in index.html already stamped data-theme on <html>
    // before first paint; this just brings the Rust state into sync with it.
    {
        let state = state.clone();
        use_effect_with((), move |_| {
            let stored = web_sys::window()
                .and_then(|w| w.local_storage().ok().flatten())
                .and_then(|s| s.get_item("moadim-theme").ok().flatten());
            state.dispatch(ShellAction::SetTheme(parse_theme(stored.as_deref())));
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

    let on_theme = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| {
            let new_theme = state.theme.toggle();
            // Stamp data-theme on <html> and persist to localStorage.
            if let Some(window) = web_sys::window() {
                if let Some(doc) = window.document() {
                    if let Some(el) = doc.document_element() {
                        let _ = el.set_attribute("data-theme", new_theme.as_str());
                    }
                }
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.set_item("moadim-theme", new_theme.as_str());
                }
            }
            state.dispatch(ShellAction::SetTheme(new_theme));
        })
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
            Route::Home => html! { <OverviewPage /> },
            Route::CronJobs => html! { <CronJobsPage on_toast={on_toast.clone()} /> },
            Route::Routines => html! { <RoutinesPage on_toast={on_toast.clone()} /> },
            Route::Heatmap => html! { <HeatmapPage /> },
            Route::NotFound => html! { <Redirect<Route> to={Route::Home} /> },
        })
    };

    let health = state.health.clone();
    let health_ok = state.health_ok;
    let toasts = state.toasts.clone();
    let show_shutdown = state.show_shutdown;
    let show_palette = state.show_palette;
    let theme = state.theme;

    html! {
        <>
            <Header health={health} ok={health_ok} on_refresh={on_refresh} on_stop={on_stop} on_palette={on_open_palette} on_theme={on_theme} theme={theme} />
            <Nav />
            <Switch<Route> render={switch} />
            <CommandPalette
                open={show_palette}
                on_close={on_close_palette}
                on_refresh={on_palette_refresh}
                on_stop={on_palette_stop}
            />
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
            <Link<Route> classes={classes!(cls(&Route::CronJobs))} to={Route::CronJobs}>
                { "CRON JOBS" }
            </Link<Route>>
            <Link<Route> classes={classes!(cls(&Route::Routines))} to={Route::Routines}>
                { "ROUTINES" }
            </Link<Route>>
            <Link<Route> classes={classes!(cls(&Route::Heatmap))} to={Route::Heatmap}>
                { "HEATMAP" }
            </Link<Route>>
        </nav>
    }
}

// ─── Header ───────────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct HeaderProps {
    pub health: Health,
    pub ok: bool,
    pub on_refresh: Callback<MouseEvent>,
    pub on_stop: Callback<MouseEvent>,
    pub on_palette: Callback<MouseEvent>,
    pub on_theme: Callback<MouseEvent>,
    pub theme: Theme,
}

#[function_component(Header)]
pub fn header(props: &HeaderProps) -> Html {
    let dot_class = if props.ok {
        "health-dot ok"
    } else {
        "health-dot error"
    };
    let status = props.health.status.to_uppercase();
    let version = props
        .health
        .version
        .as_ref()
        .map(|v| format!("/ v{v}"))
        .unwrap_or_default();
    let uptime = props
        .health
        .uptime_secs
        .map(|s| format!("/ UP {}", fmt_uptime(s)))
        .unwrap_or_default();

    html! {
        <header>
            <h1 class="logo">
                {"MOADIM"}
                <span class="logo-sub">{"/ CONTROL"}</span>
                <span class="logo-version">{version}</span>
            </h1>
            <div class="header-right">
                <div class="health">
                    <div class={dot_class}></div>
                    <span class="health-status">{status}</span>
                    <span class="health-uptime">{uptime}</span>
                </div>
                <button class="btn-cmdk" title="Command palette (⌘K)" aria-label="Open command palette" onclick={props.on_palette.clone()}>
                    {"⌘K"}
                </button>
                <button class="btn-theme" title={props.theme.aria_label()} aria-label={props.theme.aria_label()} onclick={props.on_theme.clone()}>
                    {props.theme.icon()}
                </button>
                <button class="btn-refresh" title="Refresh" aria-label="Refresh" onclick={props.on_refresh.clone()}>{"↻"}</button>
                <button class="btn-stop" title="Stop the server" disabled={!props.ok} onclick={props.on_stop.clone()}>{"⏻ STOP"}</button>
            </div>
        </header>
    }
}

// ─── Shutdown confirm dialog ──────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct ShutdownProps {
    pub on_cancel: Callback<()>,
    pub on_confirm: Callback<()>,
}

#[function_component(ShutdownDialog)]
pub fn shutdown_dialog(props: &ShutdownProps) -> Html {
    let on_cancel = {
        let cb = props.on_cancel.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };
    let on_confirm = {
        let cb = props.on_confirm.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };

    html! {
        <div class="overlay open">
            <div
                class="confirm-dialog"
                role="dialog"
                aria-modal="true"
                aria-labelledby="shutdown-dialog-title"
                aria-describedby="shutdown-dialog-msg"
            >
                <div id="shutdown-dialog-title" class="confirm-title">{"⏻ STOP SERVER"}</div>
                <div id="shutdown-dialog-msg" class="confirm-msg">
                    { "Stop the moadim server? Scheduled jobs and routines will not run until it is started again." }
                </div>
                <div class="confirm-acts">
                    <button class="btn btn-ghost btn-sm" onclick={on_cancel}>{"CANCEL"}</button>
                    <button class="btn btn-danger btn-sm" onclick={on_confirm}>{"STOP SERVER"}</button>
                </div>
            </div>
        </div>
    }
}

// ─── Toast stack ──────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct ToastStackProps {
    pub toasts: Vec<Toast>,
}

#[function_component(ToastStack)]
pub fn toast_stack(props: &ToastStackProps) -> Html {
    html! {
        <div class="toast-wrap" role="status" aria-live="polite" aria-atomic="false">
            { for props.toasts.iter().map(|t| {
                let cls = match t.kind { ToastKind::Ok => "toast ok", ToastKind::Err => "toast err" };
                html! {
                    <div class={cls} key={t.id}>{t.msg.clone()}</div>
                }
            }) }
        </div>
    }
}

// ─── Utilities (shared with routines + cron_jobs modules) ─────────────────────

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

fn fmt_uptime(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3_600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h {}m", secs / 3_600, (secs % 3_600) / 60)
    }
}

fn main() {
    console_log::init_with_level(log::Level::Info).unwrap_or_default();
    yew::Renderer::<App>::new().render();
}

#[cfg(test)]
mod main_tests {
    use super::{parse_theme, Theme};

    #[test]
    fn parse_theme_none_is_dark() {
        assert_eq!(parse_theme(None), Theme::Dark);
    }

    #[test]
    fn parse_theme_dark_string() {
        assert_eq!(parse_theme(Some("dark")), Theme::Dark);
    }

    #[test]
    fn parse_theme_unrecognised_is_dark() {
        assert_eq!(parse_theme(Some("system")), Theme::Dark);
        assert_eq!(parse_theme(Some("")), Theme::Dark);
    }

    #[test]
    fn parse_theme_light_string() {
        assert_eq!(parse_theme(Some("light")), Theme::Light);
    }

    #[test]
    fn theme_toggle_cycles() {
        assert_eq!(Theme::Dark.toggle(), Theme::Light);
        assert_eq!(Theme::Light.toggle(), Theme::Dark);
    }

    #[test]
    fn theme_as_str_round_trips() {
        assert_eq!(Theme::Dark.as_str(), "dark");
        assert_eq!(Theme::Light.as_str(), "light");
        assert_eq!(parse_theme(Some(Theme::Dark.as_str())), Theme::Dark);
        assert_eq!(parse_theme(Some(Theme::Light.as_str())), Theme::Light);
    }

    #[test]
    fn theme_icon_and_label_nonempty() {
        assert!(!Theme::Dark.icon().is_empty());
        assert!(!Theme::Light.icon().is_empty());
        assert!(!Theme::Dark.aria_label().is_empty());
        assert!(!Theme::Light.aria_label().is_empty());
    }

    #[test]
    fn theme_default_is_dark() {
        assert_eq!(Theme::default(), Theme::Dark);
    }
}
