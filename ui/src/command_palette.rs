//! The command palette (⌘K / Ctrl-K): a global, keyboard-first launcher that
//! fuzzy-searches every navigable destination — the pages plus every routine —
//! and jumps to it on Enter. It is the power-user complement to the nav tabs:
//! no mouse, no remembering which tab a routine lives under, just type a few
//! characters and go.
//!
//! Best practice (Superhuman / Linear / VS Code command palettes, and the
//! WAI-ARIA combobox+listbox pattern): bind to the de-facto ⌘K shortcut, show
//! all destinations before the user types, fuzzy-match against a title *and*
//! aliases (agent, schedule, id), group results by category, and drive the
//! whole interaction from the keyboard (↑/↓ to move, Home/End to jump, Enter to
//! launch, Esc to dismiss) while exposing it to assistive tech via
//! `role="combobox"`/`role="listbox"` and `aria-activedescendant`.
//!
//! It reads the existing `/api/v1/routines` endpoint — no backend change. All
//! match/rank/build logic lives in pure, host-tested functions below (see
//! `command_palette_tests.rs`); the component is a thin shell that fetches the
//! records, ranks them against the query, and renders.

use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlElement, HtmlInputElement};
use yew::prelude::*;
use yew_router::prelude::*;

use crate::command_palette_match::{
    badge_for, build_commands, clamp_selection, last_index, next_index, prev_index, rank,
    route_for, CmdKind, RouteKind,
};
#[cfg(test)]
use crate::command_palette_match::{fuzzy_score, routine_subtitle, schedule_label, Command};
use crate::overview::fetch_routines;
use crate::routines::Routine;
use crate::Route;

// ─── Component ─────────────────────────────────────────────────────────────────

/// Loaded records the palette ranks over.
#[derive(Clone, PartialEq, Default)]
struct Records {
    routines: Vec<Routine>,
}

#[derive(Properties, PartialEq)]
pub struct PaletteProps {
    /// Whether the palette overlay is shown.
    pub open: bool,
    /// Invoked when the palette asks to close (Esc, backdrop click, or launch).
    pub on_close: Callback<()>,
    /// Runs the "Refresh" action (re-poll health).
    pub on_refresh: Callback<()>,
    /// Runs the "Stop Server" action (open the shutdown confirmation).
    pub on_stop: Callback<()>,
    /// Runs the "Toggle Theme" action (switch light/dark mode).
    pub on_toggle_theme: Callback<()>,
}

/// The ⌘K command palette overlay. Mounted permanently by the shell; renders
/// nothing while closed. Opening it fetches the records, focuses the input, and
/// resets the query/selection.
#[function_component(CommandPalette)]
pub fn command_palette(props: &PaletteProps) -> Html {
    let navigator = use_navigator();
    let records = use_state(Records::default);
    let query = use_state(String::new);
    let selected = use_state(|| 0usize);
    let input_ref = use_node_ref();

    // On each open: re-fetch the records (so a job created elsewhere shows up),
    // reset the query/selection, and move focus into the input. The effect
    // re-runs whenever `open` flips.
    {
        let records = records.clone();
        let query = query.clone();
        let selected = selected.clone();
        let input_ref = input_ref.clone();
        let open = props.open;
        use_effect_with(open, move |_| {
            if open {
                query.set(String::new());
                selected.set(0);
                if let Some(input) = input_ref.cast::<HtmlElement>() {
                    let _ = input.focus();
                }
                spawn_local(async move {
                    let routines = fetch_routines().await.unwrap_or_default();
                    records.set(Records { routines });
                });
            }
            || ()
        });
    }

    let commands = build_commands(&records.routines);
    let order = rank(&commands, &query);
    let sel = clamp_selection(*selected, order.len());

    let launch = {
        let navigator = navigator.clone();
        let on_close = props.on_close.clone();
        let on_refresh = props.on_refresh.clone();
        let on_stop = props.on_stop.clone();
        let on_toggle_theme = props.on_toggle_theme.clone();
        let commands = commands.clone();
        let order = order.clone();
        Callback::from(move |row: usize| {
            if let Some(command) = order.get(row).and_then(|&idx| commands.get(idx)) {
                match command.kind {
                    CmdKind::ActionRefresh => on_refresh.emit(()),
                    CmdKind::ActionStop => on_stop.emit(()),
                    CmdKind::ActionToggleTheme => on_toggle_theme.emit(()),
                    kind => {
                        if let (Some(nav), Some(route_kind)) = (navigator.clone(), route_for(kind))
                        {
                            let route = match route_kind {
                                RouteKind::Home => Route::Home,
                                RouteKind::Routines => Route::Routines,
                                RouteKind::Heatmap => Route::Heatmap,
                                RouteKind::Settings => Route::Settings,
                            };
                            nav.push(&route);
                        }
                    }
                }
            }
            on_close.emit(());
        })
    };

    let on_input = {
        let query = query.clone();
        let selected = selected.clone();
        Callback::from(move |event: InputEvent| {
            let input: HtmlInputElement = event.target_unchecked_into();
            query.set(input.value());
            selected.set(0);
        })
    };

    let on_keydown = {
        let selected = selected.clone();
        let on_close = props.on_close.clone();
        let launch = launch.clone();
        let len = order.len();
        Callback::from(move |event: KeyboardEvent| match event.key().as_str() {
            "ArrowDown" => {
                event.prevent_default();
                selected.set(next_index(sel, len));
            }
            "ArrowUp" => {
                event.prevent_default();
                selected.set(prev_index(sel));
            }
            "Home" => {
                event.prevent_default();
                selected.set(0);
            }
            "End" => {
                event.prevent_default();
                selected.set(last_index(len));
            }
            "Enter" => {
                event.prevent_default();
                launch.emit(sel);
            }
            "Escape" => {
                event.prevent_default();
                on_close.emit(());
            }
            _ => {}
        })
    };

    let on_backdrop = {
        let on_close = props.on_close.clone();
        Callback::from(move |_: MouseEvent| on_close.emit(()))
    };
    // Clicks inside the dialog must not bubble to the backdrop-close handler.
    let stop = Callback::from(|event: MouseEvent| event.stop_propagation());

    if !props.open {
        return html! {};
    }

    let rows = html! {
        { for order.iter().enumerate().map(|(row, &cmd_idx)| {
            let command = &commands[cmd_idx];
            let active = row == sel;
            let row_cls = if active { "cmdk-row active" } else { "cmdk-row" };
            let badge = badge_for(command.kind);
            let badge_cls = match command.kind {
                CmdKind::Routine => "kind-badge routine",
                CmdKind::ActionRefresh
                | CmdKind::ActionStop
                | CmdKind::ActionToggleTheme => "kind-badge action",
                _ => "kind-badge nav",
            };
            let launch = launch.clone();
            let selected = selected.clone();
            let onclick = Callback::from(move |_: MouseEvent| launch.emit(row));
            let onmouseenter = Callback::from(move |_: MouseEvent| selected.set(row));
            html! {
                <li
                    id={format!("cmdk-opt-{row}")}
                    class={row_cls}
                    role="option"
                    aria-selected={active.to_string()}
                    {onclick}
                    {onmouseenter}
                >
                    <span class={badge_cls}>{badge}</span>
                    <span class="cmdk-row-text">
                        <span class="cmdk-row-title">{command.title.clone()}</span>
                        <span class="cmdk-row-sub">{command.subtitle.clone()}</span>
                    </span>
                </li>
            }
        }) }
    };

    let active_id = (!order.is_empty()).then(|| format!("cmdk-opt-{sel}"));

    html! {
        <div class="overlay open" onclick={on_backdrop}>
            <div
                class="cmdk"
                role="dialog"
                aria-modal="true"
                aria-label="Command palette"
                onclick={stop}
            >
                <div class="cmdk-search">
                    <span class="cmdk-prompt" aria-hidden="true">{"›"}</span>
                    <input
                        ref={input_ref}
                        class="cmdk-input"
                        type="text"
                        placeholder="Search pages, routines…"
                        autocomplete="off"
                        spellcheck="false"
                        role="combobox"
                        aria-expanded="true"
                        aria-controls="cmdk-listbox"
                        aria-autocomplete="list"
                        aria-activedescendant={active_id}
                        value={(*query).clone()}
                        oninput={on_input}
                        onkeydown={on_keydown}
                    />
                    <span class="cmdk-hint" aria-hidden="true">{"ESC"}</span>
                </div>
                {
                    if order.is_empty() {
                        html! {
                            <div class="cmdk-empty">
                                <div class="empty-msg">{"NO MATCHES"}</div>
                                <div class="empty-sub">{"no page or routine matches"}</div>
                            </div>
                        }
                    } else {
                        html! {
                            <ul id="cmdk-listbox" class="cmdk-list" role="listbox" aria-label="Results">
                                {rows}
                            </ul>
                        }
                    }
                }
                <div class="cmdk-foot" aria-hidden="true">
                    <span><span class="cmdk-key">{"↑↓"}</span>{" navigate"}</span>
                    <span><span class="cmdk-key">{"↵"}</span>{" open"}</span>
                    <span><span class="cmdk-key">{"esc"}</span>{" close"}</span>
                </div>
            </div>
        </div>
    }
}

#[cfg(test)]
#[path = "command_palette_tests.rs"]
mod command_palette_tests;
