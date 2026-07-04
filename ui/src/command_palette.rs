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

use crate::overview::fetch_routines;
use crate::routines::Routine;
use crate::Route;

/// What a palette entry points at — a fixed page, or an entity that lives on a
/// page. Kept free of `yew_router::Route` so the ranking/build logic is fully
/// host-testable; the component maps each variant to a concrete route.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CmdKind {
    /// Jump to the OVERVIEW page.
    NavOverview,
    /// Jump to the ROUTINES page.
    NavRoutines,
    /// Jump to the HEATMAP page.
    NavHeatmap,
    /// A specific routine (lands on the ROUTINES page).
    Routine,
    /// Re-poll server health (the header's ↻ action).
    ActionRefresh,
    /// Open the stop-server confirmation (the header's ⏻ action).
    ActionStop,
    /// Toggle the light/dark theme (persisted to localStorage).
    ActionToggleTheme,
}

/// The destination page a [`CmdKind`] resolves to, independent of the wasm
/// router so it can be asserted on the host.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum RouteKind {
    /// The OVERVIEW page (`/`).
    Home,
    /// The ROUTINES page (`/routines`).
    Routines,
    /// The HEATMAP page (`/heatmap`).
    Heatmap,
}

/// One searchable destination in the palette.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Command {
    /// What this entry points at.
    pub kind: CmdKind,
    /// Primary label shown and matched first.
    pub title: String,
    /// Secondary line (schedule description / route hint).
    pub subtitle: String,
    /// Extra terms folded into the fuzzy haystack (agent, handler, id, …) so a
    /// match on an alias still surfaces the entry.
    pub keywords: String,
}

/// The page a command navigates to, or `None` for an action command (which
/// runs a callback instead of navigating).
pub(crate) fn route_for(kind: CmdKind) -> Option<RouteKind> {
    match kind {
        CmdKind::NavOverview => Some(RouteKind::Home),
        CmdKind::NavRoutines | CmdKind::Routine => Some(RouteKind::Routines),
        CmdKind::NavHeatmap => Some(RouteKind::Heatmap),
        CmdKind::ActionRefresh | CmdKind::ActionStop | CmdKind::ActionToggleTheme => None,
    }
}

/// The short category badge text for a command (used in the row and as the
/// group separator label).
pub(crate) fn badge_for(kind: CmdKind) -> &'static str {
    match kind {
        CmdKind::NavOverview | CmdKind::NavRoutines | CmdKind::NavHeatmap => "GO",
        CmdKind::Routine => "ROUTINE",
        CmdKind::ActionRefresh | CmdKind::ActionStop | CmdKind::ActionToggleTheme => "ACTION",
    }
}

/// Score how well `query` fuzzy-matches `text`: both are compared
/// case-insensitively, and `query` must appear as an ordered subsequence of
/// `text`. A higher score is a better match. Returns `None` when `query` is not
/// a subsequence; an empty (or whitespace-only) query matches everything with a
/// neutral score of `0`, so the unfiltered list keeps its natural order.
///
/// Bonuses reward the matches users perceive as "good": a hit at the very start
/// of the text, a hit right after a word boundary, and runs of consecutive
/// characters. Longer texts are mildly penalized so a tight match on a short
/// label outranks a scattered one on a long string.
pub(crate) fn fuzzy_score(text: &str, query: &str) -> Option<i32> {
    let needle = query.trim();
    if needle.is_empty() {
        return Some(0);
    }
    let hay: Vec<char> = text.to_lowercase().chars().collect();
    let pins: Vec<char> = needle.to_lowercase().chars().collect();
    let mut score = 0i32;
    let mut cursor = 0usize;
    let mut prev: Option<usize> = None;
    for &needle_ch in &pins {
        let mut hit = None;
        while cursor < hay.len() {
            let hay_ch = hay[cursor];
            cursor += 1;
            if hay_ch == needle_ch {
                hit = Some(cursor - 1);
                break;
            }
        }
        let pos = hit?;
        score += 1;
        if pos == 0 {
            score += 10; // start of string
        } else if !hay[pos - 1].is_alphanumeric() {
            score += 6; // start of a word
        }
        if let Some(prev_pos) = prev {
            if pos == prev_pos + 1 {
                score += 8; // consecutive run
            }
        }
        prev = Some(pos);
    }
    score -= hay.len() as i32 / 16;
    Some(score)
}

/// Best fuzzy score for a command against `query`, matching the title at full
/// weight and the keyword aliases at a slight discount so a title hit wins a
/// tie. `None` when neither field matches.
fn command_score(command: &Command, query: &str) -> Option<i32> {
    let title = fuzzy_score(&command.title, query);
    let alias = fuzzy_score(&command.keywords, query).map(|raw| raw - 4);
    match (title, alias) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (left, right) => left.or(right),
    }
}

/// Indices of `commands` that match `query`, best-first. Ties keep the input
/// order (which is already grouped: pages, then routines), so an empty query
/// returns every command in its natural grouping.
pub(crate) fn rank(commands: &[Command], query: &str) -> Vec<usize> {
    let mut scored: Vec<(usize, i32)> = commands
        .iter()
        .enumerate()
        .filter_map(|(idx, command)| command_score(command, query).map(|score| (idx, score)))
        .collect();
    scored.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    scored.into_iter().map(|(idx, _)| idx).collect()
}

/// Build the full command list: the pages first, then one entry per routine.
/// Subtitles prefer the server's human schedule description and fall back to
/// the raw expression.
pub(crate) fn build_commands(routines: &[Routine]) -> Vec<Command> {
    let mut commands = vec![
        Command {
            kind: CmdKind::NavOverview,
            title: "Overview".into(),
            subtitle: "Fleet summary & upcoming runs".into(),
            keywords: "home dashboard kpi summary landing".into(),
        },
        Command {
            kind: CmdKind::NavRoutines,
            title: "Routines".into(),
            subtitle: "Manage agent-driven routines".into(),
            keywords: "agents automation".into(),
        },
        Command {
            kind: CmdKind::NavHeatmap,
            title: "Heatmap".into(),
            subtitle: "7-day × 24-hour fire-density grid".into(),
            keywords: "schedule density grid busy collisions calendar".into(),
        },
        Command {
            kind: CmdKind::ActionRefresh,
            title: "Refresh".into(),
            subtitle: "Re-poll server health".into(),
            keywords: "reload health status action".into(),
        },
        Command {
            kind: CmdKind::ActionStop,
            title: "Stop Server".into(),
            subtitle: "Shut the moadim server down".into(),
            keywords: "shutdown halt kill quit action".into(),
        },
        Command {
            kind: CmdKind::ActionToggleTheme,
            title: "Toggle Theme".into(),
            subtitle: "Switch between dark and light mode".into(),
            keywords: "theme light dark mode toggle appearance action".into(),
        },
    ];
    for routine in routines {
        commands.push(Command {
            kind: CmdKind::Routine,
            title: routine.title.clone(),
            subtitle: routine_subtitle(routine),
            keywords: format!(
                "{} {} {} routine",
                routine.id, routine.agent, routine.schedule
            ),
        });
    }
    commands
}

/// Subtitle for a routine command: schedule label optionally suffixed with
/// comma-separated status tags so health issues are visible in search results.
pub(crate) fn routine_subtitle(routine: &Routine) -> String {
    let sched = schedule_label(&routine.schedule_description, &routine.schedule);
    let mut tags: Vec<&str> = Vec::new();
    if !routine.enabled {
        tags.push("DISABLED");
    } else if routine.skip_runs.is_some_and(|n| n > 0) {
        tags.push("SNOOZED");
    } else if !routine.agent_registered {
        tags.push("AGENT MISSING");
    }
    if routine.flag_count > 0 {
        tags.push("FLAGS");
    }
    if tags.is_empty() {
        sched
    } else {
        format!("{sched} — {}", tags.join(", "))
    }
}

/// The human schedule description when present, else the raw expression, else a
/// dash so a row never renders an empty subtitle.
fn schedule_label(human: &Option<String>, raw: &str) -> String {
    match human {
        Some(text) if !text.is_empty() => text.clone(),
        _ if !raw.trim().is_empty() => raw.to_string(),
        _ => "—".into(),
    }
}

/// Clamp `selected` to a valid row index for a result list of `len` rows: stays
/// at `0` when empty, otherwise never points past the last row.
pub(crate) fn clamp_selection(selected: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        selected.min(len - 1)
    }
}

/// Next selection index when pressing ↓: advances by one but never past the
/// last row (no wrap), and stays at `0` for an empty list.
pub(crate) fn next_index(selected: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        (selected + 1).min(len - 1)
    }
}

/// Previous selection index when pressing ↑: retreats by one, saturating at the
/// first row (no wrap).
pub(crate) fn prev_index(selected: usize) -> usize {
    selected.saturating_sub(1)
}

/// Last selection index when pressing End: the final row, or `0` when empty.
pub(crate) fn last_index(len: usize) -> usize {
    len.saturating_sub(1)
}

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
