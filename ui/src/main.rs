use gloo_net::http::Request;
use gloo_timers::future::TimeoutFuture;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;

// ─── Types (mirror server API exactly) ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CronJob {
    pub id: String,
    pub schedule: String,
    pub handler: String,
    pub metadata: Json,
    pub enabled: bool,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub last_triggered_at: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
pub struct Health {
    pub status: String,
    pub uptime_secs: Option<u64>,
    pub running: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateRequest {
    pub schedule: String,
    pub handler: String,
    pub metadata: Json,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct UpdateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

// ─── App state ────────────────────────────────────────────────────────────────

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

#[derive(Debug, Clone, PartialEq)]
pub enum Modal {
    None,
    Create,
    Edit(String),
    ConfirmDelete { id: String, handler: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppState {
    pub jobs: Vec<CronJob>,
    pub health: Health,
    pub health_ok: bool,
    pub loading: bool,
    pub modal: Modal,
    pub toasts: Vec<Toast>,
    pub next_toast: u32,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            jobs: vec![],
            health: Health::default(),
            health_ok: false,
            loading: true,
            modal: Modal::None,
            toasts: vec![],
            next_toast: 0,
        }
    }
}

pub enum AppAction {
    JobsLoaded(Vec<CronJob>),
    HealthLoaded { health: Health, ok: bool },
    OpenCreate,
    OpenEdit(String),
    OpenConfirmDelete { id: String, handler: String },
    CloseModal,
    UpsertJob(CronJob),
    RemoveJob(String),
    AddToast { msg: String, kind: ToastKind },
    DismissToast(u32),
}

impl Reducible for AppState {
    type Action = AppAction;

    fn reduce(self: std::rc::Rc<Self>, action: Self::Action) -> std::rc::Rc<Self> {
        let mut s = (*self).clone();
        match action {
            AppAction::JobsLoaded(jobs) => {
                s.jobs = jobs;
                s.loading = false;
            }
            AppAction::HealthLoaded { health, ok } => {
                s.health = health;
                s.health_ok = ok;
            }
            AppAction::OpenCreate => s.modal = Modal::Create,
            AppAction::OpenEdit(id) => s.modal = Modal::Edit(id),
            AppAction::OpenConfirmDelete { id, handler } => {
                s.modal = Modal::ConfirmDelete { id, handler };
            }
            AppAction::CloseModal => s.modal = Modal::None,
            AppAction::UpsertJob(job) => {
                if let Some(i) = s.jobs.iter().position(|j| j.id == job.id) {
                    s.jobs[i] = job;
                } else {
                    s.jobs.push(job);
                }
            }
            AppAction::RemoveJob(id) => s.jobs.retain(|j| j.id != id),
            AppAction::AddToast { msg, kind } => {
                let id = s.next_toast;
                s.next_toast += 1;
                s.toasts.push(Toast {
                    id,
                    msg: AttrValue::from(msg),
                    kind,
                });
            }
            AppAction::DismissToast(id) => s.toasts.retain(|t| t.id != id),
        }
        s.into()
    }
}

// ─── API layer ────────────────────────────────────────────────────────────────

async fn api_list_jobs() -> Result<Vec<CronJob>, String> {
    Request::get("/cron-jobs")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Vec<CronJob>>()
        .await
        .map_err(|e| e.to_string())
}

async fn api_health() -> Result<Health, String> {
    Request::get("/health")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Health>()
        .await
        .map_err(|e| e.to_string())
}

async fn api_create(req: &CreateRequest) -> Result<CronJob, String> {
    let resp = Request::post("/cron-jobs")
        .json(req)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<CronJob>().await.map_err(|e| e.to_string())
}

async fn api_update(id: &str, req: &UpdateRequest) -> Result<CronJob, String> {
    let resp = Request::put(&format!("/cron-jobs/{id}"))
        .json(req)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<CronJob>().await.map_err(|e| e.to_string())
}

async fn api_delete(id: &str) -> Result<(), String> {
    let resp = Request::delete(&format!("/cron-jobs/{id}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.status() == 204 || resp.ok() {
        Ok(())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}

async fn api_trigger(id: &str) -> Result<CronJob, String> {
    let resp = Request::post(&format!("/cron-jobs/{id}/trigger"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<CronJob>().await.map_err(|e| e.to_string())
}

// ─── Root component ───────────────────────────────────────────────────────────

#[function_component(App)]
pub fn app() -> Html {
    let state = use_reducer(AppState::default);

    // Load jobs + poll health on mount
    {
        let state = state.clone();
        use_effect_with((), move |_| {
            let state = state.clone();
            spawn_local(async move {
                match api_list_jobs().await {
                    Ok(jobs) => state.dispatch(AppAction::JobsLoaded(jobs)),
                    Err(e) => state.dispatch(AppAction::AddToast {
                        msg: format!("Failed to load jobs: {e}"),
                        kind: ToastKind::Err,
                    }),
                }
                poll_health(state).await;
            });
        });
    }

    // Health poll loop every 30 s
    {
        let state = state.clone();
        use_effect_with((), move |_| {
            let state = state.clone();
            spawn_local(async move {
                loop {
                    TimeoutFuture::new(30_000).await;
                    poll_health(state.clone()).await;
                }
            });
        });
    }

    let on_refresh = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| {
            let state = state.clone();
            spawn_local(async move {
                match api_list_jobs().await {
                    Ok(jobs) => state.dispatch(AppAction::JobsLoaded(jobs)),
                    Err(e) => state.dispatch(AppAction::AddToast {
                        msg: format!("Refresh failed: {e}"),
                        kind: ToastKind::Err,
                    }),
                }
                poll_health(state).await;
            });
        })
    };

    let on_new = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| state.dispatch(AppAction::OpenCreate))
    };

    let on_edit = {
        let state = state.clone();
        Callback::from(move |id: String| state.dispatch(AppAction::OpenEdit(id)))
    };

    let on_ask_delete = {
        let state = state.clone();
        Callback::from(move |(id, handler): (String, String)| {
            state.dispatch(AppAction::OpenConfirmDelete { id, handler });
        })
    };

    let on_trigger = {
        let state = state.clone();
        Callback::from(move |id: String| {
            let state = state.clone();
            spawn_local(async move {
                match api_trigger(&id).await {
                    Ok(job) => {
                        state.dispatch(AppAction::UpsertJob(job));
                        state.dispatch(AppAction::AddToast {
                            msg: "Job triggered".into(),
                            kind: ToastKind::Ok,
                        });
                    }
                    Err(e) => state.dispatch(AppAction::AddToast {
                        msg: format!("Trigger failed: {e}"),
                        kind: ToastKind::Err,
                    }),
                }
            })
        })
    };

    let on_toggle = {
        let state = state.clone();
        Callback::from(move |(id, enabled): (String, bool)| {
            let state = state.clone();
            spawn_local(async move {
                let req = UpdateRequest {
                    enabled: Some(enabled),
                    ..Default::default()
                };
                match api_update(&id, &req).await {
                    Ok(job) => {
                        state.dispatch(AppAction::UpsertJob(job));
                        state.dispatch(AppAction::AddToast {
                            msg: if enabled {
                                "Job enabled".into()
                            } else {
                                "Job disabled".into()
                            },
                            kind: ToastKind::Ok,
                        });
                    }
                    Err(e) => state.dispatch(AppAction::AddToast {
                        msg: format!("Toggle failed: {e}"),
                        kind: ToastKind::Err,
                    }),
                }
            })
        })
    };

    let on_close = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(AppAction::CloseModal))
    };

    // Snapshot modal at render time so on_save sees the right mode
    let current_modal = state.modal.clone();
    let on_save = {
        let state = state.clone();
        Callback::from(move |req: CreateRequest| {
            let state = state.clone();
            let modal = current_modal.clone();
            spawn_local(async move {
                match &modal {
                    Modal::Edit(id) => {
                        let upd = UpdateRequest {
                            schedule: Some(req.schedule),
                            handler: Some(req.handler),
                            metadata: Some(req.metadata),
                            enabled: Some(req.enabled),
                        };
                        match api_update(id, &upd).await {
                            Ok(job) => {
                                state.dispatch(AppAction::UpsertJob(job));
                                state.dispatch(AppAction::CloseModal);
                                state.dispatch(AppAction::AddToast {
                                    msg: "Job updated".into(),
                                    kind: ToastKind::Ok,
                                });
                            }
                            Err(e) => state.dispatch(AppAction::AddToast {
                                msg: format!("Update failed: {e}"),
                                kind: ToastKind::Err,
                            }),
                        }
                    }
                    _ => match api_create(&req).await {
                        Ok(job) => {
                            state.dispatch(AppAction::UpsertJob(job));
                            state.dispatch(AppAction::CloseModal);
                            state.dispatch(AppAction::AddToast {
                                msg: "Job created".into(),
                                kind: ToastKind::Ok,
                            });
                        }
                        Err(e) => state.dispatch(AppAction::AddToast {
                            msg: format!("Create failed: {e}"),
                            kind: ToastKind::Err,
                        }),
                    },
                }
            })
        })
    };

    let on_confirm_delete = {
        let state = state.clone();
        Callback::from(move |id: String| {
            let state = state.clone();
            spawn_local(async move {
                match api_delete(&id).await {
                    Ok(()) => {
                        state.dispatch(AppAction::RemoveJob(id));
                        state.dispatch(AppAction::CloseModal);
                        state.dispatch(AppAction::AddToast {
                            msg: "Job deleted".into(),
                            kind: ToastKind::Ok,
                        });
                    }
                    Err(e) => state.dispatch(AppAction::AddToast {
                        msg: format!("Delete failed: {e}"),
                        kind: ToastKind::Err,
                    }),
                }
            })
        })
    };

    let on_dismiss_toast = {
        let state = state.clone();
        Callback::from(move |id: u32| state.dispatch(AppAction::DismissToast(id)))
    };

    let jobs = state.jobs.clone();
    let health = state.health.clone();
    let health_ok = state.health_ok;
    let loading = state.loading;
    let modal = state.modal.clone();
    let toasts = state.toasts.clone();

    let edit_job = match &modal {
        Modal::Edit(id) => jobs.iter().find(|j| j.id == *id).cloned(),
        _ => None,
    };

    html! {
        <>
            <Header health={health} ok={health_ok} on_refresh={on_refresh} />
            <main>
                <StatsBar jobs={jobs.clone()} />
                <div class="section-hd">
                    <div class="section-label">{"SCHEDULED JOBS"}</div>
                    <button class="btn btn-primary btn-sm" onclick={on_new}>{"+ NEW JOB"}</button>
                </div>
                <JobTable
                    jobs={jobs}
                    loading={loading}
                    on_edit={on_edit}
                    on_delete={on_ask_delete}
                    on_toggle={on_toggle}
                    on_trigger={on_trigger}
                />
            </main>
            {
                match &modal {
                    Modal::Create | Modal::Edit(_) => html! {
                        <JobModal
                            editing={edit_job}
                            on_close={on_close.clone()}
                            on_save={on_save}
                        />
                    },
                    Modal::ConfirmDelete { id, handler } => html! {
                        <ConfirmDialog
                            job_id={id.clone()}
                            handler={handler.clone()}
                            on_cancel={on_close}
                            on_confirm={on_confirm_delete}
                        />
                    },
                    Modal::None => html! {},
                }
            }
            <ToastStack toasts={toasts} on_dismiss={on_dismiss_toast} />
        </>
    }
}

async fn poll_health(state: UseReducerHandle<AppState>) {
    match api_health().await {
        Ok(health) => {
            let ok = health.running;
            state.dispatch(AppAction::HealthLoaded { health, ok });
        }
        Err(_) => state.dispatch(AppAction::HealthLoaded {
            health: Health {
                status: "offline".into(),
                running: false,
                uptime_secs: None,
            },
            ok: false,
        }),
    }
}

// ─── Header ───────────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct HeaderProps {
    pub health: Health,
    pub ok: bool,
    pub on_refresh: Callback<MouseEvent>,
}

#[function_component(Header)]
pub fn header(props: &HeaderProps) -> Html {
    let dot_class = if props.ok {
        "health-dot ok"
    } else {
        "health-dot error"
    };
    let status = props.health.status.to_uppercase();
    let uptime = props
        .health
        .uptime_secs
        .map(|s| format!("/ UP {}", fmt_uptime(s)))
        .unwrap_or_default();

    html! {
        <header>
            <div class="logo">
                {"MOADIM"}
                <span class="logo-sub">{"/ CRON CONTROL"}</span>
            </div>
            <div class="header-right">
                <div class="health">
                    <div class={dot_class}></div>
                    <span class="health-status">{status}</span>
                    <span class="health-uptime">{uptime}</span>
                </div>
                <button class="btn-refresh" title="Refresh" onclick={props.on_refresh.clone()}>{"↻"}</button>
            </div>
        </header>
    }
}

// ─── Stats bar ────────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct StatsProps {
    pub jobs: Vec<CronJob>,
}

#[function_component(StatsBar)]
pub fn stats_bar(props: &StatsProps) -> Html {
    let total = props.jobs.len();
    let enabled = props.jobs.iter().filter(|j| j.enabled).count();
    let disabled = total - enabled;

    html! {
        <div class="stats">
            <div class="stat-card all">
                <div class="stat-label">{"TOTAL JOBS"}</div>
                <div class="stat-val">{total}</div>
            </div>
            <div class="stat-card enabled">
                <div class="stat-label">{"ENABLED"}</div>
                <div class="stat-val c-accent">{enabled}</div>
            </div>
            <div class="stat-card disabled">
                <div class="stat-label">{"DISABLED"}</div>
                <div class="stat-val c-amber">{disabled}</div>
            </div>
        </div>
    }
}

// ─── Job table ────────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct JobTableProps {
    pub jobs: Vec<CronJob>,
    pub loading: bool,
    pub on_edit: Callback<String>,
    pub on_delete: Callback<(String, String)>,
    pub on_toggle: Callback<(String, bool)>,
    pub on_trigger: Callback<String>,
}

#[function_component(JobTable)]
pub fn job_table(props: &JobTableProps) -> Html {
    if props.loading {
        return html! {
            <div class="table-wrap">
                <div class="empty"><div class="spinner"></div></div>
            </div>
        };
    }
    if props.jobs.is_empty() {
        return html! {
            <div class="table-wrap">
                <div class="empty">
                    <div class="empty-icon">{"⧗"}</div>
                    <div class="empty-msg">{"NO JOBS SCHEDULED"}</div>
                    <div class="empty-sub">{"press + NEW JOB to create one"}</div>
                </div>
            </div>
        };
    }

    html! {
        <div class="table-wrap">
            <table>
                <thead>
                    <tr>
                        <th>{"ID"}</th>
                        <th>{"SCHEDULE"}</th>
                        <th>{"HANDLER"}</th>
                        <th>{"METADATA"}</th>
                        <th>{"ENABLED"}</th>
                        <th>{"UPDATED"}</th>
                        <th></th>
                    </tr>
                </thead>
                <tbody>
                    { for props.jobs.iter().map(|job| html! {
                        <JobRow
                            key={job.id.clone()}
                            job={job.clone()}
                            on_edit={props.on_edit.clone()}
                            on_delete={props.on_delete.clone()}
                            on_toggle={props.on_toggle.clone()}
                            on_trigger={props.on_trigger.clone()}
                        />
                    }) }
                </tbody>
            </table>
        </div>
    }
}

// ─── Job row ──────────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct JobRowProps {
    pub job: CronJob,
    pub on_edit: Callback<String>,
    pub on_delete: Callback<(String, String)>,
    pub on_toggle: Callback<(String, bool)>,
    pub on_trigger: Callback<String>,
}

#[function_component(JobRow)]
pub fn job_row(props: &JobRowProps) -> Html {
    let job = &props.job;
    let id_short = format!("{}…", &job.id[..8.min(job.id.len())]);
    let (cron_ok, cron_text) = describe_cron(&job.schedule);
    let meta = meta_preview(&job.metadata);
    let updated = reltime(job.updated_at);

    let on_edit = {
        let cb = props.on_edit.clone();
        let id = job.id.clone();
        Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
    };
    let on_delete = {
        let cb = props.on_delete.clone();
        let id = job.id.clone();
        let handler = job.handler.clone();
        Callback::from(move |_: MouseEvent| cb.emit((id.clone(), handler.clone())))
    };
    let on_toggle = {
        let cb = props.on_toggle.clone();
        let id = job.id.clone();
        let enabled = job.enabled;
        Callback::from(move |_: Event| cb.emit((id.clone(), !enabled)))
    };
    let on_trigger = {
        let cb = props.on_trigger.clone();
        let id = job.id.clone();
        Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
    };

    let last_run = job.last_triggered_at
        .map(|t| format!("↻ {}", reltime(t)))
        .unwrap_or_default();

    let _ = cron_ok; // used only for styling if desired later
    html! {
        <tr>
            <td><span class="cell-id" title={job.id.clone()}>{id_short}</span></td>
            <td>
                <div class="cell-schedule">{&job.schedule}</div>
                <div class="cell-schedule-human">{cron_text}</div>
            </td>
            <td><span class="cell-handler">{&job.handler}</span></td>
            <td><span class="cell-meta">{meta}</span></td>
            <td>
                <label class="toggle">
                    <input type="checkbox" checked={job.enabled} onchange={on_toggle} />
                    <div class="toggle-track"></div>
                </label>
            </td>
            <td>
                <div class="cell-time">{updated}</div>
                if !last_run.is_empty() {
                    <div class="cell-triggered">{last_run}</div>
                }
            </td>
            <td>
                <div class="row-actions">
                    <button class="act-btn run" title="Run now" onclick={on_trigger}>{"▶"}</button>
                    <button class="act-btn edit" onclick={on_edit}>{"EDIT"}</button>
                    <button class="act-btn del" onclick={on_delete}>{"✕"}</button>
                </div>
            </td>
        </tr>
    }
}

// ─── Job modal ────────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct JobModalProps {
    pub editing: Option<CronJob>,
    pub on_close: Callback<()>,
    pub on_save: Callback<CreateRequest>,
}

#[function_component(JobModal)]
pub fn job_modal(props: &JobModalProps) -> Html {
    let schedule = use_state(|| {
        props
            .editing
            .as_ref()
            .map(|j| j.schedule.clone())
            .unwrap_or_default()
    });
    let handler = use_state(|| {
        props
            .editing
            .as_ref()
            .map(|j| j.handler.clone())
            .unwrap_or_default()
    });
    let meta_raw = use_state(|| {
        props
            .editing
            .as_ref()
            .and_then(|j| {
                if j.metadata.is_null() {
                    None
                } else {
                    serde_json::to_string_pretty(&j.metadata).ok()
                }
            })
            .unwrap_or_default()
    });
    let enabled = use_state(|| {
        props
            .editing
            .as_ref()
            .map(|j| j.enabled)
            .unwrap_or(true)
    });
    let meta_err = use_state(String::new);
    let saving = use_state(|| false);

    let (cron_ok, cron_text) = describe_cron(&schedule);

    let is_edit = props.editing.is_some();
    let title = if is_edit { "EDIT JOB" } else { "NEW JOB" };
    let save_label = if is_edit { "SAVE CHANGES" } else { "CREATE JOB" };

    let on_schedule = {
        let schedule = schedule.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            schedule.set(input.value());
        })
    };
    let on_handler = {
        let handler = handler.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            handler.set(input.value());
        })
    };
    let on_meta = {
        let meta_raw = meta_raw.clone();
        let meta_err = meta_err.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let val = input.value();
            if val.trim().is_empty() {
                meta_err.set(String::new());
            } else if let Err(err) = serde_json::from_str::<Json>(&val) {
                meta_err.set(format!("↳ {err}"));
            } else {
                meta_err.set(String::new());
            }
            meta_raw.set(val);
        })
    };
    let on_enabled = {
        let enabled = enabled.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            enabled.set(input.checked());
        })
    };

    let set_preset = |val: &'static str| {
        let schedule = schedule.clone();
        Callback::from(move |_: MouseEvent| schedule.set(val.to_string()))
    };

    let on_close_click = {
        let cb = props.on_close.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };
    let on_save_click = {
        let schedule = schedule.clone();
        let handler = handler.clone();
        let meta_raw = meta_raw.clone();
        let meta_err = meta_err.clone();
        let enabled = enabled.clone();
        let saving = saving.clone();
        let cb = props.on_save.clone();
        Callback::from(move |_: MouseEvent| {
            if !meta_err.is_empty() {
                return;
            }
            let metadata = if meta_raw.trim().is_empty() {
                Json::Null
            } else {
                serde_json::from_str(&*meta_raw).unwrap_or(Json::Null)
            };
            saving.set(true);
            cb.emit(CreateRequest {
                schedule: (*schedule).clone(),
                handler: (*handler).clone(),
                metadata,
                enabled: *enabled,
            });
        })
    };

    let preview_class = if schedule.is_empty() {
        "cron-preview"
    } else if cron_ok {
        "cron-preview ok"
    } else {
        "cron-preview bad"
    };

    let meta_class = if !meta_err.is_empty() {
        "form-input invalid"
    } else {
        "form-input"
    };

    html! {
        <div class="overlay">
            <div class="modal">
                <div class="modal-hd">
                    <div class="modal-title">{title}</div>
                    <button class="modal-x" onclick={on_close_click.clone()}>{"✕"}</button>
                </div>
                <div class="modal-body">
                    <div class="form-group">
                        <label class="form-label">
                            {"SCHEDULE "}
                            <span class="form-required">{"*"}</span>
                        </label>
                        <input
                            class="form-input"
                            type="text"
                            placeholder="sec min hour dom month dow year"
                            value={(*schedule).clone()}
                            oninput={on_schedule}
                            autocomplete="off"
                            spellcheck="false"
                        />
                        <div class="cron-presets">
                            { for [
                                ("@daily", "@daily"), ("@hourly", "@hourly"),
                                ("@weekly", "@weekly"), ("@monthly", "@monthly"),
                                ("0 0 9 * * 1-5 *", "weekdays 9am"),
                                ("0 */15 * * * * *", "every 15min"),
                                ("0 0 * * * * *", "every hour"),
                                ("0 0 0 1 * * *", "monthly"),
                            ].iter().map(|(val, label)| html! {
                                <button class="preset-btn" onclick={set_preset(val)}>{*label}</button>
                            }) }
                        </div>
                        <div class={preview_class}>{cron_text}</div>
                    </div>
                    <div class="form-group">
                        <label class="form-label">
                            {"HANDLER "}
                            <span class="form-required">{"*"}</span>
                        </label>
                        <input
                            class="form-input"
                            type="text"
                            placeholder="send-report"
                            value={(*handler).clone()}
                            oninput={on_handler}
                            autocomplete="off"
                            spellcheck="false"
                        />
                    </div>
                    <div class="form-group">
                        <label class="form-label">
                            {"METADATA "}
                            <span style="color:var(--text-ghost)">{"(JSON)"}</span>
                        </label>
                        <textarea
                            class={meta_class}
                            placeholder={r#"{"recipient": "team@example.com"}"#}
                            value={(*meta_raw).clone()}
                            oninput={on_meta}
                        />
                        if !meta_err.is_empty() {
                            <div class="field-err">{(*meta_err).clone()}</div>
                        }
                    </div>
                    <div class="form-group" style="margin-bottom:0">
                        <div class="toggle-row">
                            <span class="toggle-row-label">{"ENABLED"}</span>
                            <label class="toggle">
                                <input type="checkbox" checked={*enabled} onchange={on_enabled} />
                                <div class="toggle-track"></div>
                            </label>
                        </div>
                    </div>
                </div>
                <div class="modal-ft">
                    <button class="btn btn-ghost btn-sm" onclick={on_close_click}>{"CANCEL"}</button>
                    <button
                        class="btn btn-primary btn-sm"
                        onclick={on_save_click}
                        disabled={*saving}
                    >
                        { if *saving { "…" } else { save_label } }
                    </button>
                </div>
            </div>
        </div>
    }
}

// ─── Confirm dialog ───────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct ConfirmProps {
    pub job_id: String,
    pub handler: String,
    pub on_cancel: Callback<()>,
    pub on_confirm: Callback<String>,
}

#[function_component(ConfirmDialog)]
pub fn confirm_dialog(props: &ConfirmProps) -> Html {
    let on_cancel = {
        let cb = props.on_cancel.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };
    let on_confirm = {
        let cb = props.on_confirm.clone();
        let id = props.job_id.clone();
        Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
    };

    html! {
        <div class="overlay">
            <div class="confirm-dialog">
                <div class="confirm-title">{"⚠ DELETE JOB"}</div>
                <div class="confirm-msg">
                    { format!("Delete the job running \"{}\"? This cannot be undone.", props.handler) }
                </div>
                <div class="confirm-acts">
                    <button class="btn btn-ghost btn-sm" onclick={on_cancel}>{"CANCEL"}</button>
                    <button class="btn btn-danger btn-sm" onclick={on_confirm}>{"DELETE"}</button>
                </div>
            </div>
        </div>
    }
}

// ─── Toast stack ──────────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct ToastStackProps {
    pub toasts: Vec<Toast>,
    pub on_dismiss: Callback<u32>,
}

#[function_component(ToastStack)]
pub fn toast_stack(props: &ToastStackProps) -> Html {
    html! {
        <div class="toast-wrap">
            { for props.toasts.iter().map(|t| {
                let cls = match t.kind { ToastKind::Ok => "toast ok", ToastKind::Err => "toast err" };
                html! {
                    <div class={cls} key={t.id}>{t.msg.clone()}</div>
                }
            }) }
        </div>
    }
}

// ─── Utilities ────────────────────────────────────────────────────────────────

/// Returns (is_valid, human description) for a cron expression.
fn describe_cron(expr: &str) -> (bool, String) {
    let s = expr.trim();
    if s.is_empty() {
        return (false, "— enter a cron expression —".into());
    }
    let specials = [
        ("@yearly", "Yearly — January 1st at midnight"),
        ("@annually", "Yearly — January 1st at midnight"),
        ("@monthly", "Monthly — 1st at midnight"),
        ("@weekly", "Weekly — every Sunday at midnight"),
        ("@daily", "Daily — at midnight"),
        ("@midnight", "Daily — at midnight"),
        ("@hourly", "Every hour — at minute 0"),
    ];
    for (key, desc) in specials {
        if s.eq_ignore_ascii_case(key) {
            return (true, desc.into());
        }
    }
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() < 5 || parts.len() > 7 {
        return (
            false,
            format!("Invalid: expected 5–7 fields, got {}", parts.len()),
        );
    }
    // Basic positional description (sec min hour dom month dow [year])
    let (min, hour) = match parts.len() {
        5 => (parts[0], parts[1]),
        _ => (parts[1], parts[2]),
    };
    let time_desc = if hour == "*" && min == "*" {
        "every minute".into()
    } else if hour == "*" {
        if let Some(n) = min.strip_prefix("*/") {
            format!("every {n} minutes")
        } else {
            format!("minute {min} of every hour")
        }
    } else if let Some(n) = hour.strip_prefix("*/") {
        format!("every {n} hours")
    } else if let (Ok(h), Ok(m)) = (hour.parse::<u32>(), min.parse::<u32>()) {
        let ap = if h >= 12 { "PM" } else { "AM" };
        let dh = if h == 0 { 12 } else if h > 12 { h - 12 } else { h };
        format!("{dh}:{m:02} {ap}")
    } else {
        format!("{hour}:{min}")
    };
    (true, time_desc)
}

fn meta_preview(v: &Json) -> String {
    match v {
        Json::Null => "—".into(),
        Json::Object(o) if o.is_empty() => "{}".into(),
        Json::Object(o) => {
            let k = o.len();
            if k == 1 {
                format!("{{{}}}", o.keys().next().unwrap())
            } else {
                format!("{{{k} keys}}")
            }
        }
        other => other.to_string().chars().take(24).collect(),
    }
}

fn reltime(ts: u64) -> String {
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
