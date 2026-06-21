use croner::Cron;
use gloo_net::http::Request;
use gloo_timers::future::TimeoutFuture;
use serde::Deserialize;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

mod cron_jobs;
mod day_timeline;
mod routines;
use cron_jobs::CronJobsPage;
use routines::RoutinesPage;

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
}

pub enum ShellAction {
    HealthLoaded { health: Health, ok: bool },
    AddToast { msg: String, kind: ToastKind },
    OpenShutdown,
    CloseShutdown,
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

    let on_toast = {
        let state = state.clone();
        Callback::from(move |(msg, kind): (String, ToastKind)| {
            state.dispatch(ShellAction::AddToast { msg, kind })
        })
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
            Route::Home => html! { <Redirect<Route> to={Route::CronJobs} /> },
            Route::CronJobs => html! { <CronJobsPage on_toast={on_toast.clone()} /> },
            Route::Routines => html! { <RoutinesPage on_toast={on_toast.clone()} /> },
            Route::NotFound => html! { <Redirect<Route> to={Route::CronJobs} /> },
        })
    };

    let health = state.health.clone();
    let health_ok = state.health_ok;
    let toasts = state.toasts.clone();
    let show_shutdown = state.show_shutdown;

    html! {
        <>
            <Header health={health} ok={health_ok} on_refresh={on_refresh} on_stop={on_stop} />
            <Nav />
            <Switch<Route> render={switch} />
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
    // Home redirects to CronJobs, so treat it as the cron-jobs tab for highlighting.
    let cls = |target: &Route| {
        let active = route == *target || (route == Route::Home && *target == Route::CronJobs);
        if active {
            "tab-btn active"
        } else {
            "tab-btn"
        }
    };
    html! {
        <nav class="tabs">
            <Link<Route> classes={classes!(cls(&Route::CronJobs))} to={Route::CronJobs}>
                { "CRON JOBS" }
            </Link<Route>>
            <Link<Route> classes={classes!(cls(&Route::Routines))} to={Route::Routines}>
                { "ROUTINES" }
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
