//! Full-text search + status / agent / machine / repository facets and sort
//! controls for the routine table.

use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

use super::filter::{
    AgentFacet, RepositoryFacet, RoutineFilter, RoutineMachineFacet, RoutineStatusFacet, TagFacet,
    RMACHINE_ANY, RMACHINE_UNASSIGNED,
};

// ─── Filter & sort bar ────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct FilterSortBarProps {
    pub filter: RoutineFilter,
    /// Distinct agent names across all routines, for the agent-facet options.
    pub agents: Vec<String>,
    /// Distinct machine ids across all routines, for the machine-facet options.
    pub machines: Vec<String>,
    /// Distinct repository URLs across all routines, for the repository-facet options.
    pub repositories: Vec<String>,
    /// Distinct tags across all routines, for the tag-facet options. Hidden when empty.
    pub tags: Vec<String>,
    /// Count after filtering / total loaded — rendered as "Showing N of M".
    pub shown: usize,
    pub total: usize,
    /// NodeRef forwarded from the page so the `/` shortcut can focus this input.
    pub search_ref: NodeRef,
    pub on_query: Callback<String>,
    pub on_status: Callback<RoutineStatusFacet>,
    pub on_agent: Callback<AgentFacet>,
    pub on_machine: Callback<RoutineMachineFacet>,
    pub on_repository: Callback<RepositoryFacet>,
    pub on_tag: Callback<TagFacet>,
    pub on_clear: Callback<()>,
}

/// Full-text search + status / agent / machine facets + sort controls for the routine table.
#[function_component(FilterSortBar)]
pub fn filter_sort_bar(props: &FilterSortBarProps) -> Html {
    let on_input = {
        let cb = props.on_query.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            cb.emit(input.value());
        })
    };
    let on_status_change = {
        let cb = props.on_status.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            cb.emit(RoutineStatusFacet::from_str(&select.value()));
        })
    };
    let on_agent_change = {
        let cb = props.on_agent.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            cb.emit(AgentFacet::from_value(&select.value()));
        })
    };
    let on_machine_change = {
        let cb = props.on_machine.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            cb.emit(RoutineMachineFacet::from_value(&select.value()));
        })
    };
    let on_repository_change = {
        let cb = props.on_repository.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            cb.emit(RepositoryFacet::from_value(&select.value()));
        })
    };
    let on_tag_change = {
        let cb = props.on_tag.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            cb.emit(TagFacet::from_value(&select.value()));
        })
    };
    let on_clear = {
        let cb = props.on_clear.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };
    let status_val = props.filter.status.as_str();
    let agent_val = props.filter.agent.as_value();
    let machine_val = props.filter.machine.as_value();
    let repository_val = props.filter.repository.as_value();
    let tag_val = props.filter.tag.as_value();
    let active = props.filter.is_active();

    html! {
        <div class="filter-bar">
            <div class="filter-field">
                <input
                    ref={props.search_ref.clone()}
                    type="text"
                    class="filter-input"
                    placeholder="Search routines…  ( / )"
                    aria-label="Search routines"
                    value={props.filter.query.clone()}
                    oninput={on_input}
                />
                <span class="filter-label">{"STATUS"}</span>
                <select class="filter-select" aria-label="Status filter" onchange={on_status_change}>
                    <option value="all" selected={status_val == "all"}>{"All"}</option>
                    <option value="enabled" selected={status_val == "enabled"}>{"Enabled"}</option>
                    <option value="disabled" selected={status_val == "disabled"}>{"Disabled"}</option>
                    <option value="dormant" selected={status_val == "dormant"}>{"Dormant"}</option>
                    <option value="due" selected={status_val == "due"}>{"Due soon"}</option>
                    <option value="snoozed" selected={status_val == "snoozed"}>{"Snoozed"}</option>
                    <option value="flagged" selected={status_val == "flagged"}>{"Flagged"}</option>
                    <option value="agent-unreg" selected={status_val == "agent-unreg"}>{"Agent unregistered"}</option>
                </select>
                <span class="filter-label">{"AGENT"}</span>
                <select class="filter-select" aria-label="Agent filter" onchange={on_agent_change}>
                    <option value={AgentFacet::AGENT_ALL} selected={agent_val == AgentFacet::AGENT_ALL}>{"Any"}</option>
                    { for props.agents.iter().map(|a| html! {
                        <option value={a.clone()} selected={agent_val == *a}>{a.clone()}</option>
                    }) }
                </select>
                <span class="filter-label">{"MACHINE"}</span>
                <select class="filter-select" aria-label="Machine filter" onchange={on_machine_change}>
                    <option value={RMACHINE_ANY} selected={machine_val == RMACHINE_ANY}>{"Any"}</option>
                    <option value={RMACHINE_UNASSIGNED}
                        selected={machine_val == RMACHINE_UNASSIGNED}>{"None"}</option>
                    { for props.machines.iter().map(|m| html! {
                        <option value={m.clone()} selected={machine_val == *m}>{m.clone()}</option>
                    }) }
                </select>
                <span class="filter-label">{"REPOSITORY"}</span>
                <select class="filter-select" aria-label="Repository filter" onchange={on_repository_change}>
                    <option value={RepositoryFacet::REPOSITORY_ALL}
                        selected={repository_val == RepositoryFacet::REPOSITORY_ALL}>{"Any"}</option>
                    { for props.repositories.iter().map(|r| html! {
                        <option value={r.clone()} selected={repository_val == *r}>{r.clone()}</option>
                    }) }
                </select>
                {
                    if props.tags.is_empty() {
                        html! {}
                    } else {
                        html! {
                            <>
                                <span class="filter-label">{"TAG"}</span>
                                <select class="filter-select" aria-label="Tag filter" onchange={on_tag_change}>
                                    <option value={TagFacet::TAG_ALL} selected={tag_val == TagFacet::TAG_ALL}>{"Any"}</option>
                                    { for props.tags.iter().map(|t| html! {
                                        <option value={t.clone()} selected={tag_val == *t}>{t.clone()}</option>
                                    }) }
                                </select>
                            </>
                        }
                    }
                }
            </div>
            <div class="filter-field">
                <span class="filter-count">
                    {format!("Showing {} of {}", props.shown, props.total)}
                </span>
                {
                    if active {
                        html! {
                            <button class="btn btn-ghost btn-sm" onclick={on_clear}
                                title="Clear all filters">{"CLEAR"}</button>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>
        </div>
    }
}
