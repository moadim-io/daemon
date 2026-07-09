//! Routine create/edit form: field state, validation, cron preview, and the
//! text<->list conversions for repositories and tags.

use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

use crate::describe_cron_live;
use crate::machines::MachinesPicker;

use super::model::{api_agents, CreateRoutineRequest, Repository, Routine, AVAILABLE_AGENTS};

// ─── Form (create page + edit modal) ──────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct FormProps {
    pub editing: Option<Routine>,
    pub on_cancel: Callback<()>,
    pub on_save: Callback<CreateRoutineRequest>,
}

/// Title for a cloned routine: prepend "Copy of " when the original title does not
/// already start with that prefix, preventing "Copy of Copy of …" accumulation.
pub(crate) fn clone_title(title: &str) -> String {
    const PREFIX: &str = "Copy of ";
    if title.starts_with(PREFIX) {
        title.to_string()
    } else {
        format!("{PREFIX}{title}")
    }
}

/// Serialize repositories as one `url [branch]` line each for the textarea.
fn repos_to_text(repos: &[Repository]) -> String {
    repos
        .iter()
        .map(|r| match &r.branch {
            Some(b) if !b.is_empty() => format!("{} {}", r.repository, b),
            _ => r.repository.clone(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parse a textarea of `url [branch]` lines back into repositories.
fn text_to_repos(text: &str) -> Vec<Repository> {
    text.lines()
        .filter_map(|line| {
            let mut it = line.split_whitespace();
            let repository = it.next()?.to_string();
            let branch = it.next().map(std::string::ToString::to_string);
            Some(Repository { repository, branch })
        })
        .collect()
}

/// Join tags into a single comma-separated string for the input field.
fn tags_to_text(tags: &[String]) -> String {
    tags.join(", ")
}

/// Split a comma-separated input into trimmed, non-empty tags.
fn text_to_tags(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect()
}

/// Parse a TTL textarea value into seconds. Blank/whitespace → `None` (use the server default);
/// a valid non-negative integer → `Some(secs)`; anything else → `None`.
fn parse_ttl(raw: &str) -> Option<u64> {
    let t = raw.trim();
    if t.is_empty() {
        None
    } else {
        t.parse::<u64>().ok()
    }
}

/// Render a routine's TTL for display: `None` shows the server default, otherwise a compact
/// duration (`7d`, `12h`, `30m`, `45s`).
pub(crate) fn format_ttl(ttl_secs: Option<u64>) -> String {
    match ttl_secs {
        None => "default".to_string(),
        Some(0) => "0s".to_string(),
        Some(s) if s % 86_400 == 0 => format!("{}d", s / 86_400),
        Some(s) if s % 3_600 == 0 => format!("{}h", s / 3_600),
        Some(s) if s % 60 == 0 => format!("{}m", s / 60),
        Some(s) => format!("{s}s"),
    }
}

/// (seconds, label) pairs for the WORKBENCH TTL preset buttons, mirroring the cron
/// SCHEDULE presets: 1 hour, 1 day, 7 days, 30 days.
const TTL_PRESETS: [(&str, &str); 4] = [
    ("3600", "1h"),
    ("86400", "1d"),
    ("604800", "7d"),
    ("2592000", "30d"),
];

#[function_component(RoutineForm)]
pub fn routine_form(props: &FormProps) -> Html {
    let editing = props.editing.clone();
    let is_edit = editing.is_some();

    let title = use_state(|| {
        editing
            .as_ref()
            .map(|r| r.title.clone())
            .unwrap_or_default()
    });
    let schedule = use_state(|| {
        editing
            .as_ref()
            .map(|r| r.schedule.clone())
            .unwrap_or_default()
    });
    let agent = use_state(|| {
        editing
            .as_ref()
            .map_or_else(|| "claude".to_string(), |r| r.agent.clone())
    });
    // Agent options fetched from `GET /agents`; seed with the built-in list so the select is never
    // empty before the request resolves or if it fails.
    let agents = use_state(|| {
        AVAILABLE_AGENTS
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
    });
    {
        let agents = agents.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                if let Ok(list) = api_agents().await {
                    if !list.is_empty() {
                        agents.set(list);
                    }
                }
            });
            || ()
        });
    }
    // Free-text model override; blank means "use the agent's own default".
    let model = use_state(|| {
        editing
            .as_ref()
            .and_then(|r| r.model.clone())
            .unwrap_or_default()
    });
    let prompt = use_state(|| {
        editing
            .as_ref()
            .map(|r| r.prompt.clone())
            .unwrap_or_default()
    });
    // A very short (≤5 line) "why" for the routine; blank means unset.
    let goal = use_state(|| {
        editing
            .as_ref()
            .and_then(|r| r.goal.clone())
            .unwrap_or_default()
    });
    let repos_raw = use_state(|| {
        editing
            .as_ref()
            .map(|r| repos_to_text(&r.repositories))
            .unwrap_or_default()
    });
    let machines = use_state(|| {
        editing
            .as_ref()
            .map(|r| r.machines.clone())
            .unwrap_or_default()
    });
    let enabled = use_state(|| editing.as_ref().is_none_or(|r| r.enabled));
    // Comma-separated tags; blank means no tags.
    let tags_raw = use_state(|| {
        editing
            .as_ref()
            .map(|r| tags_to_text(&r.tags))
            .unwrap_or_default()
    });
    // Blank means "use the server default"; otherwise the workbench TTL in seconds.
    let ttl_raw = use_state(|| {
        editing
            .as_ref()
            .and_then(|r| r.ttl_secs)
            .map(|s| s.to_string())
            .unwrap_or_default()
    });
    let saving = use_state(|| false);

    let (cron_ok, cron_text) = describe_cron_live(&schedule);

    let on_title = {
        let title = title.clone();
        Callback::from(move |e: InputEvent| {
            let i: HtmlInputElement = e.target_unchecked_into();
            title.set(i.value());
        })
    };
    let on_schedule = {
        let schedule = schedule.clone();
        Callback::from(move |e: InputEvent| {
            let i: HtmlInputElement = e.target_unchecked_into();
            schedule.set(i.value());
        })
    };
    let on_agent = {
        let agent = agent.clone();
        Callback::from(move |e: Event| {
            let s: HtmlSelectElement = e.target_unchecked_into();
            agent.set(s.value());
        })
    };
    let on_model = {
        let model = model.clone();
        Callback::from(move |e: InputEvent| {
            let i: HtmlInputElement = e.target_unchecked_into();
            model.set(i.value());
        })
    };
    let on_prompt = {
        let prompt = prompt.clone();
        Callback::from(move |e: InputEvent| {
            let i: HtmlInputElement = e.target_unchecked_into();
            prompt.set(i.value());
        })
    };
    let on_goal = {
        let goal = goal.clone();
        Callback::from(move |e: InputEvent| {
            let i: HtmlInputElement = e.target_unchecked_into();
            goal.set(i.value());
        })
    };
    let on_repos = {
        let repos_raw = repos_raw.clone();
        Callback::from(move |e: InputEvent| {
            let i: HtmlInputElement = e.target_unchecked_into();
            repos_raw.set(i.value());
        })
    };
    let on_machines = {
        let machines = machines.clone();
        Callback::from(move |next: Vec<String>| machines.set(next))
    };
    let on_enabled = {
        let enabled = enabled.clone();
        Callback::from(move |e: Event| {
            let i: HtmlInputElement = e.target_unchecked_into();
            enabled.set(i.checked());
        })
    };
    let on_ttl = {
        let ttl_raw = ttl_raw.clone();
        Callback::from(move |e: InputEvent| {
            let i: HtmlInputElement = e.target_unchecked_into();
            ttl_raw.set(i.value());
        })
    };
    let on_tags = {
        let tags_raw = tags_raw.clone();
        Callback::from(move |e: InputEvent| {
            let i: HtmlInputElement = e.target_unchecked_into();
            tags_raw.set(i.value());
        })
    };

    let set_preset = |val: &'static str| {
        let schedule = schedule.clone();
        Callback::from(move |_: MouseEvent| schedule.set(val.to_string()))
    };

    let set_ttl_preset = |val: &'static str| {
        let ttl_raw = ttl_raw.clone();
        Callback::from(move |_: MouseEvent| ttl_raw.set(val.to_string()))
    };

    let on_cancel_click = {
        let cb = props.on_cancel.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };

    let can_save = !title.trim().is_empty()
        && !schedule.trim().is_empty()
        && !agent.trim().is_empty()
        && !prompt.trim().is_empty();

    let on_save_click = {
        let title = title.clone();
        let schedule = schedule.clone();
        let agent = agent.clone();
        let model = model.clone();
        let prompt = prompt.clone();
        let goal = goal.clone();
        let repos_raw = repos_raw.clone();
        let machines = machines.clone();
        let enabled = enabled.clone();
        let ttl_raw = ttl_raw.clone();
        let tags_raw = tags_raw.clone();
        let saving = saving.clone();
        let cb = props.on_save.clone();
        Callback::from(move |_: MouseEvent| {
            if !can_save {
                return;
            }
            saving.set(true);
            cb.emit(CreateRoutineRequest {
                schedule: (*schedule).clone(),
                title: (*title).clone(),
                agent: (*agent).clone(),
                model: Some((*model).clone()).filter(|text| !text.trim().is_empty()),
                prompt: (*prompt).clone(),
                goal: Some((*goal).clone()).filter(|text| !text.trim().is_empty()),
                repositories: text_to_repos(&repos_raw),
                machines: (*machines).clone(),
                enabled: *enabled,
                ttl_secs: parse_ttl(&ttl_raw),
                tags: text_to_tags(&tags_raw),
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

    let submit_label = if *saving {
        "…"
    } else if is_edit {
        "SAVE CHANGES"
    } else {
        "CREATE ROUTINE"
    };

    let body = html! {
        <div class="modal-body">
            <div class="form-group">
                <label class="form-label">{"TITLE "}<span class="form-required">{"*"}</span></label>
                <input class="form-input" type="text" placeholder="nightly triage"
                    value={(*title).clone()} oninput={on_title} autocomplete="off" spellcheck="false" />
            </div>
            <div class="form-group">
                <label class="form-label">{"SCHEDULE "}<span class="form-required">{"*"}</span></label>
                <input class="form-input" type="text" placeholder="sec min hour dom month dow year"
                    value={(*schedule).clone()} oninput={on_schedule} autocomplete="off" spellcheck="false" />
                <div class="cron-presets">
                    { for [
                        ("@daily", "@daily"), ("@hourly", "@hourly"),
                        ("@weekly", "@weekly"), ("@monthly", "@monthly"),
                        ("0 0 9 * * 1-5 *", "weekdays 9am"),
                        ("0 0 * * * * *", "every hour"),
                    ].iter().map(|(val, label)| html! {
                        <button class="preset-btn" onclick={set_preset(val)}>{*label}</button>
                    }) }
                </div>
                <div class={preview_class}>{cron_text}</div>
            </div>
            <div class="form-group">
                <label class="form-label">{"AGENT "}<span class="form-required">{"*"}</span></label>
                <select class="form-input" onchange={on_agent}>
                    { for agents.iter().map(|name| html! {
                        <option value={name.clone()} selected={*agent == *name}>{name.clone()}</option>
                    }) }
                </select>
            </div>
            <div class="form-group">
                <label class="form-label">
                    {"MODEL "}
                    <span style="color:var(--text-ghost)">{"(optional; blank = agent default)"}</span>
                </label>
                <input class="form-input" type="text" placeholder="claude-sonnet-4-6"
                    value={(*model).clone()} oninput={on_model} autocomplete="off" spellcheck="false" />
            </div>
            <div class="form-group">
                <label class="form-label">{"PROMPT "}<span class="form-required">{"*"}</span></label>
                <textarea class="form-input" placeholder="Review open PRs and summarize…"
                    value={(*prompt).clone()} oninput={on_prompt} />
            </div>
            <div class="form-group">
                <label class="form-label">
                    {"GOAL "}
                    <span style="color:var(--text-ghost)">{"(optional; ≤5 lines — the why)"}</span>
                </label>
                <textarea class="form-input" placeholder="Keep the PR backlog under control…"
                    value={(*goal).clone()} oninput={on_goal} />
            </div>
            <div class="form-group">
                <label class="form-label">
                    {"REPOSITORIES "}
                    <span style="color:var(--text-ghost)">{"(one url [branch] per line)"}</span>
                </label>
                <textarea class="form-input" placeholder={"https://github.com/org/repo main"}
                    value={(*repos_raw).clone()} oninput={on_repos} />
            </div>
            <MachinesPicker value={(*machines).clone()} on_change={on_machines} />
            <div class="form-group">
                <label class="form-label">
                    {"TAGS "}
                    <span style="color:var(--text-ghost)">{"(comma-separated)"}</span>
                </label>
                <input class="form-input" type="text" placeholder="triage, nightly"
                    value={(*tags_raw).clone()} oninput={on_tags} autocomplete="off" spellcheck="false" />
            </div>
            <div class="form-group">
                <label class="form-label">
                    {"WORKBENCH TTL "}
                    <span style="color:var(--text-ghost)">{"(seconds; blank = server default)"}</span>
                </label>
                <input class="form-input" type="number" min="0" placeholder="604800"
                    value={(*ttl_raw).clone()} oninput={on_ttl} autocomplete="off" spellcheck="false" />
                <div class="ttl-presets">
                    { for TTL_PRESETS.iter().map(|(val, label)| html! {
                        <button class="preset-btn" onclick={set_ttl_preset(val)}>{*label}</button>
                    }) }
                </div>
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
    };

    let footer = html! {
        <div class="modal-ft">
            <button class="btn btn-ghost btn-sm" onclick={on_cancel_click.clone()}>{"CANCEL"}</button>
            <button class="btn btn-primary btn-sm" onclick={on_save_click} disabled={*saving || !can_save}>
                { submit_label }
            </button>
        </div>
    };

    if is_edit {
        html! {
            <div class="overlay open">
                <div class="modal">
                    <div class="modal-hd">
                        <div class="modal-title">{"EDIT ROUTINE"}</div>
                        <button class="modal-x" title="Close" aria-label="Close" onclick={on_cancel_click}>{"✕"}</button>
                    </div>
                    {body}
                    {footer}
                </div>
            </div>
        }
    } else {
        html! {
            <main class="create-page">
                <div class="page-hd">
                    <button class="btn btn-ghost btn-sm" onclick={on_cancel_click}>{"← BACK"}</button>
                    <div class="page-title">{"NEW ROUTINE"}</div>
                </div>
                <div class="page-card">
                    {body}
                    {footer}
                </div>
            </main>
        }
    }
}

#[cfg(test)]
#[path = "form_tests.rs"]
mod form_tests;
