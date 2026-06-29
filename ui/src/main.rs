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

async fn api_get_machine() -> Result<String, String> {
    #[derive(serde::Deserialize)]
    struct Resp {
        name: String,
    }
    Request::get("/api/v1/machine")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Resp>()
        .await
        .map(|r| r.name)
        .map_err(|e| e.to_string())
}

async fn api_put_machine(name: &str) -> Result<String, String> {
    #[derive(serde::Serialize)]
    struct Body<'a> {
        name: &'a str,
    }
    #[derive(serde::Deserialize)]
    struct Resp {
        name: String,
    }
    let resp = Request::put("/api/v1/machine")
        .header("content-type", "application/json")
        .body(serde_json::to_string(&Body { name }).unwrap())
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        resp.json::<Resp>()
            .await
            .map(|r| r.name)
            .map_err(|e| e.to_string())
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
        Callback::from(move |(name, on_done): (String, Callback<Result<(), String>>)| {
            let state = state.clone();
            spawn_local(async move {
                match api_put_machine(&name).await {
                    Ok(new_name) => {
                        state.dispatch(ShellAction::MachineName { name: new_name.clone() });
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
        })
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
    let theme_icon = if props.light { "☀" } else { "🌙" };
    let theme_title = if props.light {
        "Switch to dark mode"
    } else {
        "Switch to light mode"
    };

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

// ─── Rename machine dialog ────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct RenameMachineProps {
    /// Current machine name (pre-fills the input).
    pub current: String,
    pub on_cancel: Callback<()>,
    /// Emits `(new_name, done_callback)`. The caller fires the API call; the `done_callback` is
    /// invoked with `Ok(())` on success so the dialog can reset its busy state.
    pub on_confirm: Callback<(String, Callback<Result<(), String>>)>,
}

#[function_component(RenameMachineDialog)]
pub fn rename_machine_dialog(props: &RenameMachineProps) -> Html {
    let draft = use_state(|| props.current.clone());
    let busy = use_state(|| false);

    let on_input = {
        let draft = draft.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            draft.set(input.value());
        })
    };

    let on_cancel = {
        let cb = props.on_cancel.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };

    let on_save = {
        let draft = draft.clone();
        let busy = busy.clone();
        let cb = props.on_confirm.clone();
        Callback::from(move |_: MouseEvent| {
            let name = (*draft).trim().to_string();
            if name.is_empty() {
                return;
            }
            busy.set(true);
            let busy2 = busy.clone();
            let done = Callback::from(move |_: Result<(), String>| {
                busy2.set(false);
            });
            cb.emit((name, done));
        })
    };

    let is_busy = *busy;
    let is_empty = draft.trim().is_empty();

    html! {
        <div class="overlay open">
            <div
                class="confirm-dialog"
                role="dialog"
                aria-modal="true"
                aria-labelledby="rename-machine-title"
            >
                <div id="rename-machine-title" class="confirm-title">{"RENAME MACHINE"}</div>
                <div class="confirm-msg">
                    <label class="form-label" for="rename-machine-input">{"MACHINE NAME"}</label>
                    <input id="rename-machine-input"
                        class="form-input"
                        type="text"
                        value={(*draft).clone()}
                        oninput={on_input}
                        disabled={is_busy}
                        autocomplete="off"
                        spellcheck="false"
                    />
                </div>
                <div class="confirm-acts">
                    <button class="btn btn-ghost btn-sm" onclick={on_cancel} disabled={is_busy}>{"CANCEL"}</button>
                    <button class="btn btn-primary btn-sm" onclick={on_save}
                        disabled={is_busy || is_empty}>
                        { if is_busy { "SAVING…" } else { "RENAME" } }
                    </button>
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
