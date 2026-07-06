//! The Overview page's "NEEDS ATTENTION" triage: the fault classification for
//! enabled-but-misconfigured entities and the table that renders them. Split
//! out of `overview.rs` to keep that file under the line-count gate; used only
//! by `OverviewPage`.

use chrono::{DateTime, Local};
use yew::prelude::*;
use yew_router::prelude::*;

use crate::overview::{Kind, SchedSource};
use crate::schedule::next_fire_after;
use crate::Route;

/// Why an enabled entity needs attention. Listed in triage priority order: a
/// dormant entity outranks a dead schedule, which outranks a missing agent,
/// which outranks open flags, so each entity surfaces its single most
/// fundamental fault (see [`attention_reason`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum AttentionReason {
    /// Enabled but assigned to no machine — it fires nowhere.
    Dormant,
    /// Targets a machine, but the schedule yields no future fire (empty,
    /// invalid, or a one-shot already in the past) — it never runs again.
    DeadSchedule,
    /// A routine whose agent is not registered — every run errors out.
    AgentUnregistered,
    /// Agent raised one or more flags during a run — needs human review.
    HasOpenFlags,
}

impl AttentionReason {
    /// Triage priority; lower sorts first.
    pub(crate) fn rank(self) -> u8 {
        match self {
            AttentionReason::Dormant => 0,
            AttentionReason::DeadSchedule => 1,
            AttentionReason::AgentUnregistered => 2,
            AttentionReason::HasOpenFlags => 3,
        }
    }

    /// Short uppercase badge label for the ISSUE column.
    pub(crate) fn badge(self) -> &'static str {
        match self {
            AttentionReason::Dormant => "DORMANT",
            AttentionReason::DeadSchedule => "DEAD SCHEDULE",
            AttentionReason::AgentUnregistered => "AGENT MISSING",
            AttentionReason::HasOpenFlags => "OPEN FLAGS",
        }
    }

    /// Human explanation of the operational consequence.
    pub(crate) fn detail(self) -> &'static str {
        match self {
            AttentionReason::Dormant => "assigned to no machine — fires nowhere",
            AttentionReason::DeadSchedule => "schedule has no future fire — never runs again",
            AttentionReason::AgentUnregistered => "agent not registered — every run errors",
            AttentionReason::HasOpenFlags => "agent raised flags during a run — needs review",
        }
    }
}

/// One enabled-but-misconfigured entity surfaced in the NEEDS ATTENTION panel.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct AttentionItem {
    /// Always `Kind::Routine` for now.
    pub kind: Kind,
    /// Display name.
    pub label: String,
    /// The single most fundamental fault to fix.
    pub reason: AttentionReason,
    /// Open flag count; non-zero only when `reason == HasOpenFlags`.
    pub flag_count: usize,
}

/// The single most fundamental fault for an enabled `source`, or `None` when it
/// is healthy. Disabled entities are intentional and never flagged. Faults are
/// checked in priority order so each entity reports exactly one reason.
pub(crate) fn attention_reason(
    source: &SchedSource,
    now: DateTime<Local>,
) -> Option<AttentionReason> {
    if !source.enabled {
        return None;
    }
    if source.machines_empty {
        return Some(AttentionReason::Dormant);
    }
    if next_fire_after(&source.schedule, now).is_none() {
        return Some(AttentionReason::DeadSchedule);
    }
    if source.agent_registered == Some(false) {
        return Some(AttentionReason::AgentUnregistered);
    }
    if source.flag_count > 0 {
        return Some(AttentionReason::HasOpenFlags);
    }
    None
}

/// All enabled-but-misconfigured entities, worst fault first, ties broken by
/// label for a stable order.
pub(crate) fn attention_items(sources: &[SchedSource], now: DateTime<Local>) -> Vec<AttentionItem> {
    let mut items: Vec<AttentionItem> = sources
        .iter()
        .filter_map(|s| {
            attention_reason(s, now).map(|reason| AttentionItem {
                kind: s.kind,
                label: s.label.clone(),
                flag_count: s.flag_count,
                reason,
            })
        })
        .collect();
    items.sort_by(|a, b| {
        a.reason
            .rank()
            .cmp(&b.reason.rank())
            .then_with(|| a.label.cmp(&b.label))
    });
    items
}

#[derive(Properties, PartialEq)]
pub(crate) struct AttentionTableProps {
    pub(crate) items: Vec<AttentionItem>,
}

/// The NEEDS ATTENTION triage table: one row per enabled-but-broken entity,
/// worst fault first. Rendered only when `items` is non-empty (see the page),
/// so this component never has to handle the loading/empty states.
#[function_component(AttentionTable)]
pub(crate) fn attention_table(props: &AttentionTableProps) -> Html {
    html! {
        <div class="table-wrap attn-wrap">
            <table>
                <thead>
                    <tr>
                        <th>{"TYPE"}</th>
                        <th>{"NAME"}</th>
                        <th>{"ISSUE"}</th>
                        <th>{"DETAIL"}</th>
                    </tr>
                </thead>
                <tbody>
                    { for props.items.iter().enumerate().map(|(i, item)| {
                        let (badge, badge_cls, to) = match item.kind {
                            Kind::Routine => ("ROUTINE", "kind-badge routine", Route::Routines),
                        };
                        html! {
                            <tr key={i.to_string()}>
                                <td><span class={badge_cls}>{badge}</span></td>
                                <td>
                                    <Link<Route> classes={classes!("ov-name-link")} to={to}>
                                        {item.label.clone()}
                                    </Link<Route>>
                                </td>
                                <td><span class="attn-badge">{item.reason.badge()}</span></td>
                                <td class="attn-detail">{
                                    if item.reason == AttentionReason::HasOpenFlags && item.flag_count > 0 {
                                        format!("{} open flag{} — needs review", item.flag_count, if item.flag_count == 1 { "" } else { "s" })
                                    } else {
                                        item.reason.detail().to_string()
                                    }
                                }</td>
                            </tr>
                        }
                    }) }
                </tbody>
            </table>
        </div>
    }
}
