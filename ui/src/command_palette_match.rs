//! Pure matching/ranking logic behind the command palette: the searchable
//! `Command` model, fuzzy scoring, ranking, the records→`Command` builder, and
//! the keyboard selection-index helpers. Split out of `command_palette.rs` to
//! keep that file under the line-count gate; the component in that file is a
//! thin shell over these host-testable functions (see
//! `command_palette_tests.rs`).

use crate::routines::Routine;

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
    /// Jump to the SETTINGS page.
    NavSettings,
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
    /// The SETTINGS page (`/settings`).
    Settings,
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
        CmdKind::NavSettings => Some(RouteKind::Settings),
        CmdKind::ActionRefresh | CmdKind::ActionStop | CmdKind::ActionToggleTheme => None,
    }
}

/// The short category badge text for a command (used in the row and as the
/// group separator label).
pub(crate) fn badge_for(kind: CmdKind) -> &'static str {
    match kind {
        CmdKind::NavOverview
        | CmdKind::NavRoutines
        | CmdKind::NavHeatmap
        | CmdKind::NavSettings => "GO",
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
            kind: CmdKind::NavSettings,
            title: "Settings".into(),
            subtitle: "Persistent agent prompt".into(),
            keywords: "config preferences user prompt".into(),
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
                "{} {} {} {} routine",
                routine.id,
                routine.agent,
                routine.schedule,
                routine.tags.join(" ")
            ),
        });
    }
    commands
}

/// Subtitle for a routine command: schedule label optionally suffixed with
/// comma-separated status tags so health issues are visible in search results.
pub(crate) fn routine_subtitle(routine: &Routine) -> String {
    let sched = schedule_label(routine.schedule_description.as_ref(), &routine.schedule);
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
pub(crate) fn schedule_label(human: Option<&String>, raw: &str) -> String {
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
