//! Host-side unit tests for the command palette's pure logic: fuzzy scoring,
//! command ranking, the records→`Command` builder, route/badge mapping, and the
//! keyboard selection-index helpers. All deterministic and DOM-free.

use super::*;

fn routine(id: &str, title: &str, agent: &str, schedule: &str, human: Option<&str>) -> Routine {
    Routine {
        id: id.into(),
        schedule: schedule.into(),
        title: title.into(),
        agent: agent.into(),
        prompt: String::new(),
        repositories: vec![],
        machines: vec![],
        enabled: true,
        source: String::new(),
        created_at: 0,
        updated_at: 0,
        last_manual_trigger_at: None,
        last_scheduled_trigger_at: None,
        ttl_secs: None,
        tags: vec![],
        agent_registered: false,
        file_path: String::new(),
        schedule_description: human.map(Into::into),
    }
}

fn cmd(kind: CmdKind, title: &str, keywords: &str) -> Command {
    Command {
        kind,
        title: title.into(),
        subtitle: String::new(),
        keywords: keywords.into(),
    }
}

// ─── fuzzy_score ────────────────────────────────────────────────────────────

#[test]
fn empty_query_matches_with_neutral_score() {
    assert_eq!(fuzzy_score("anything", ""), Some(0));
    assert_eq!(fuzzy_score("anything", "   "), Some(0));
}

#[test]
fn non_subsequence_does_not_match() {
    assert_eq!(fuzzy_score("abc", "xyz"), None);
    assert_eq!(fuzzy_score("abc", "cab"), None); // out of order
}

#[test]
fn start_of_string_outranks_later_hit() {
    let lead = fuzzy_score("overview", "o").expect("matches");
    let mid = fuzzy_score("retro", "o").expect("matches");
    assert!(lead > mid, "leading hit {lead} should beat mid hit {mid}");
}

#[test]
fn word_boundary_hit_is_rewarded() {
    // 'j' after the space scores the word-boundary bonus.
    let boundary = fuzzy_score("nightly jobs", "j").expect("matches");
    let interior = fuzzy_score("major", "j").expect("matches");
    assert!(
        boundary > interior,
        "boundary {boundary} > interior {interior}"
    );
}

#[test]
fn consecutive_run_beats_scattered() {
    // A tight run of the query characters outscores the same characters spread
    // apart. Neutral tokens keep the spell-checker happy.
    let consecutive = fuzzy_score("zzabczz", "abc").expect("matches");
    let scattered = fuzzy_score("axbxc", "abc").expect("matches");
    assert!(
        consecutive > scattered,
        "consecutive {consecutive} > scattered {scattered}"
    );
}

#[test]
fn longer_text_is_penalized() {
    let short = fuzzy_score("ab", "a").expect("matches");
    let long = fuzzy_score(&"a".repeat(40), "a").expect("matches");
    // Same leading match, but the 40-char text loses points to the length term.
    assert!(short > long, "short {short} > long {long}");
}

#[test]
fn case_is_ignored() {
    assert!(fuzzy_score("Overview", "OVERVIEW").is_some());
}

// ─── rank (and command_score via it) ─────────────────────────────────────────

#[test]
fn empty_query_keeps_natural_order() {
    let commands = vec![
        cmd(CmdKind::NavOverview, "Overview", ""),
        cmd(CmdKind::Routine, "backup", ""),
    ];
    assert_eq!(rank(&commands, ""), vec![0, 1]);
}

#[test]
fn title_match_outranks_keyword_only_match() {
    let commands = vec![
        // index 0: matches only via keyword alias
        cmd(CmdKind::Routine, "nightly", "backup database"),
        // index 1: matches directly in the title
        cmd(CmdKind::Routine, "backup", "misc"),
    ];
    let order = rank(&commands, "backup");
    assert_eq!(order.first(), Some(&1), "title hit should rank first");
    assert_eq!(order.len(), 2);
}

#[test]
fn keyword_only_match_still_surfaces() {
    // Title has no 'z'; only the keywords do. Exercises the (None, Some) arm.
    let commands = vec![cmd(CmdKind::Routine, "alpha", "zeta")];
    assert_eq!(rank(&commands, "zeta"), vec![0]);
}

#[test]
fn title_match_with_unmatched_keywords() {
    // Exercises the (Some, None) arm: title matches, keywords do not.
    let commands = vec![cmd(CmdKind::Routine, "deploy", "xxxxx")];
    assert_eq!(rank(&commands, "deploy"), vec![0]);
}

#[test]
fn non_matching_commands_are_dropped() {
    let commands = vec![
        cmd(CmdKind::Routine, "alpha", "one"),
        cmd(CmdKind::Routine, "beta", "two"),
    ];
    assert!(rank(&commands, "zzz").is_empty());
}

// ─── build_commands ──────────────────────────────────────────────────────────

#[test]
fn build_lists_pages_then_routines() {
    let routines = vec![routine("r1", "Nightly Audit", "claude", "0 0 * * *", None)];
    let commands = build_commands(&routines);
    assert_eq!(commands.len(), 7); // 3 nav + 3 action + 1 routine
    assert_eq!(commands[0].kind, CmdKind::NavOverview);
    assert_eq!(commands[1].kind, CmdKind::NavRoutines);
    assert_eq!(commands[2].kind, CmdKind::NavHeatmap);
    assert_eq!(commands[3].kind, CmdKind::ActionRefresh);
    assert_eq!(commands[4].kind, CmdKind::ActionStop);
    assert_eq!(commands[5].kind, CmdKind::ActionToggleTheme);
    assert!(commands[5].keywords.contains("theme"));
    assert_eq!(commands[6].kind, CmdKind::Routine);
    assert_eq!(commands[6].title, "Nightly Audit");
    assert_eq!(commands[6].subtitle, "0 0 * * *"); // falls back to raw expr
    assert!(commands[6].keywords.contains("claude"));
}

#[test]
fn schedule_label_prefers_human_then_raw_then_dash() {
    assert_eq!(
        schedule_label(&Some("At noon".into()), "0 12 * * *"),
        "At noon"
    );
    // Empty human description falls through to the raw expression.
    assert_eq!(
        schedule_label(&Some(String::new()), "0 12 * * *"),
        "0 12 * * *"
    );
    assert_eq!(schedule_label(&None, "0 12 * * *"), "0 12 * * *");
    // Neither present → a dash, never an empty string.
    assert_eq!(schedule_label(&None, "   "), "—");
}

// ─── route_for / badge_for ────────────────────────────────────────────────────

#[test]
fn route_for_maps_every_kind() {
    assert_eq!(route_for(CmdKind::NavOverview), Some(RouteKind::Home));
    assert_eq!(route_for(CmdKind::NavRoutines), Some(RouteKind::Routines));
    assert_eq!(route_for(CmdKind::NavHeatmap), Some(RouteKind::Heatmap));
    assert_eq!(route_for(CmdKind::Routine), Some(RouteKind::Routines));
    // Action commands run a callback, not a navigation.
    assert_eq!(route_for(CmdKind::ActionRefresh), None);
    assert_eq!(route_for(CmdKind::ActionStop), None);
    assert_eq!(route_for(CmdKind::ActionToggleTheme), None);
}

#[test]
fn badge_for_maps_every_kind() {
    assert_eq!(badge_for(CmdKind::NavOverview), "GO");
    assert_eq!(badge_for(CmdKind::NavRoutines), "GO");
    assert_eq!(badge_for(CmdKind::NavHeatmap), "GO");
    assert_eq!(badge_for(CmdKind::Routine), "ROUTINE");
    assert_eq!(badge_for(CmdKind::ActionRefresh), "ACTION");
    assert_eq!(badge_for(CmdKind::ActionStop), "ACTION");
    assert_eq!(badge_for(CmdKind::ActionToggleTheme), "ACTION");
}

// ─── selection-index helpers ───────────────────────────────────────────────────

#[test]
fn clamp_selection_handles_empty_and_overflow() {
    assert_eq!(clamp_selection(5, 0), 0); // empty list pins to 0
    assert_eq!(clamp_selection(2, 4), 2); // in range untouched
    assert_eq!(clamp_selection(9, 4), 3); // past end clamps to last
}

#[test]
fn next_index_advances_without_wrapping() {
    assert_eq!(next_index(0, 0), 0); // empty
    assert_eq!(next_index(0, 3), 1);
    assert_eq!(next_index(2, 3), 2); // already last → stays
}

#[test]
fn prev_index_saturates_at_zero() {
    assert_eq!(prev_index(0), 0);
    assert_eq!(prev_index(3), 2);
}

#[test]
fn last_index_is_final_row_or_zero() {
    assert_eq!(last_index(0), 0);
    assert_eq!(last_index(5), 4);
}
