//! Reusable machine-targeting picker shared by the routine and cron-job forms.
//!
//! Fetches the daemon's known machine names (`GET /api/v1/machines` — every name referenced by a
//! routine or cron job, plus this machine's own identity) and renders them as toggleable chips.
//! The selection stays open-ended: a brand-new name can be added even if no entry references it
//! yet, so the picker never blocks assigning a machine the daemon hasn't seen.

use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;

/// Fetch the current machine's resolved name from the daemon.
pub async fn api_current_machine() -> Result<String, String> {
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

async fn api_machines() -> Result<Vec<String>, String> {
    Request::get("/api/v1/machines")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Vec<String>>()
        .await
        .map_err(|e| e.to_string())
}

#[derive(Properties, PartialEq)]
pub struct MachinesPickerProps {
    /// Currently selected machine names.
    pub value: Vec<String>,
    /// Emits the new (sorted, de-duplicated) selection whenever it changes.
    pub on_change: Callback<Vec<String>>,
}

#[function_component(MachinesPicker)]
pub fn machines_picker(props: &MachinesPickerProps) -> Html {
    // Known machines fetched from the daemon. Merged with the current selection below so a machine
    // that is assigned but not yet referenced elsewhere still renders as a chip.
    let known = use_state(Vec::<String>::new);
    {
        let known = known.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                if let Ok(list) = api_machines().await {
                    known.set(list);
                }
            });
            || ()
        });
    }
    let new_name = use_state(String::new);

    // Candidate chips: union of known + selected, sorted and de-duplicated.
    let mut candidates: Vec<String> = known
        .iter()
        .cloned()
        .chain(props.value.iter().cloned())
        .collect();
    candidates.sort();
    candidates.dedup();

    let toggle = {
        let on_change = props.on_change.clone();
        let value = props.value.clone();
        Callback::from(move |name: String| {
            let mut next = value.clone();
            if let Some(pos) = next.iter().position(|m| m == &name) {
                next.remove(pos);
            } else {
                next.push(name);
                next.sort();
            }
            on_change.emit(next);
        })
    };

    let on_new_input = {
        let new_name = new_name.clone();
        Callback::from(move |e: InputEvent| {
            let i: HtmlInputElement = e.target_unchecked_into();
            new_name.set(i.value());
        })
    };

    let add_new = {
        let on_change = props.on_change.clone();
        let value = props.value.clone();
        let new_name = new_name.clone();
        Callback::from(move |_: MouseEvent| {
            let name = new_name.trim().to_string();
            // Ignore blanks and names already selected; either way clear the input.
            if !name.is_empty() && !value.contains(&name) {
                let mut next = value.clone();
                next.push(name);
                next.sort();
                on_change.emit(next);
            }
            new_name.set(String::new());
        })
    };

    html! {
        <div class="form-group">
            <label class="form-label">
                {"MACHINES "}
                <span style="color:var(--text-ghost)">{"(pick targets; none = runs nowhere)"}</span>
            </label>
            if candidates.is_empty() {
                <div class="machine-empty">{"No machines known yet — add one below."}</div>
            } else {
                <div class="machine-chips">
                    { for candidates.iter().map(|name| {
                        let on = props.value.contains(name);
                        let cls = if on { "machine-chip on" } else { "machine-chip" };
                        let toggle = toggle.clone();
                        let n = name.clone();
                        html! {
                            <button type="button" class={cls}
                                onclick={Callback::from(move |_: MouseEvent| toggle.emit(n.clone()))}>
                                { name.clone() }
                            </button>
                        }
                    }) }
                </div>
            }
            <div class="machine-add">
                <input class="form-input" type="text" placeholder="add a machine name"
                    value={(*new_name).clone()} oninput={on_new_input}
                    autocomplete="off" spellcheck="false" />
                <button type="button" class="preset-btn" onclick={add_new}>{"ADD"}</button>
            </div>
        </div>
    }
}
