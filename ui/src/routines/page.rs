//! The routines list page: data loading, filter/sort/group wiring, bulk actions, and routing
//! between the list, create, edit, logs, and flags sub-views.

use chrono::{Duration, Local};
use yew::prelude::*;
use yew_router::prelude::*;

use crate::day_timeline::{DayTimeline, TimelineItem};
use crate::refresh::{RefreshControl, RefreshInterval};
use crate::ToastKind;

use super::actions::{install_crud_handlers, CrudHandlers};
use super::banner::{GlobalLockBanner, RoutineGroupBySelector, RoutineStatsBar, ViewToggle};
use super::bulk::{ConfirmDelete, RoutineBulkBar, RoutineBulkDeleteDialog};
use super::bulk_actions::{install_bulk_handlers, BulkHandlers};
use super::calendar::RoutineCalendar;
use super::filter::{
    distinct_agents, distinct_machines_r, distinct_repositories, distinct_tags, filter_routines,
    is_routine_snoozed, AgentFacet, RepositoryFacet, RoutineMachineFacet, RoutineStatusFacet,
    TagFacet, DUE_SOON_WINDOW_SECS,
};
use super::filter_bar::FilterSortBar;
use super::flags_panel::RoutineFlags;
use super::form::{clone_title, RoutineForm};
use super::history::RoutineHistory;
use super::hooks::{
    install_auto_refresh, install_current_machine_loader, install_lock_status_loader,
    install_now_ticker, install_routines_loader, install_search_hotkey,
};
use super::logs::RoutineLogs;
use super::state::{
    sort_routines, RAction, RCol, RGroupBy, RModal, RPage, RState, RView, RoutineHistoryQuery,
};
use super::table::RoutineTable;

// ─── Page component ───────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct RoutinesPageProps {
    pub on_toast: Callback<(String, ToastKind)>,
}

#[function_component(RoutinesPage)]
pub fn routines_page(props: &RoutinesPageProps) -> Html {
    let state = use_reducer(RState::default);
    let toast = props.on_toast.clone();

    // Live "now" advanced on a fixed tick so DUE SOON counts stay current.
    let now = use_state(Local::now);
    install_now_ticker(now.clone());

    // Operator-chosen auto-refresh cadence (persisted), and the `Date.now()`
    // (ms) of the last successful list load that drives the freshness cue.
    let interval = use_state(crate::refresh::load_interval);
    let updated_at = use_state(|| 0.0_f64);

    // Load on mount.
    install_routines_loader(state.clone(), toast.clone(), updated_at.clone());

    // Fetch and apply the current machine as the default machine filter.
    install_current_machine_loader(state.clone());

    // Fetch lock status on mount and whenever routines reload.
    install_lock_status_loader(state.clone());

    // Deep-link straight to a routine's HISTORY page when the URL carries a `?history=<id>`
    // query (e.g. `/routines?history=<id>`, as followed from the overview page's RECENT RUNS
    // panel), instead of always landing on the plain routine list.
    {
        let state = state.clone();
        let location = use_location();
        use_effect_with((), move |_| {
            if let Some(id) = location
                .and_then(|loc| loc.query::<RoutineHistoryQuery>().ok())
                .map(|q| q.history)
            {
                state.dispatch(RAction::GoToHistory(id));
            }
        });
    }

    // Auto-refresh loop, re-armed whenever the chosen interval changes.
    install_auto_refresh(*interval, state.clone(), toast.clone(), updated_at.clone());
    let on_set_interval = {
        let interval = interval.clone();
        Callback::from(move |next: RefreshInterval| {
            crate::refresh::save_interval(next);
            interval.set(next);
        })
    };

    let on_new = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| state.dispatch(RAction::GoToNew))
    };
    let on_cancel = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(RAction::GoToList))
    };
    let on_close = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(RAction::CloseModal))
    };
    let on_logs = {
        let state = state.clone();
        Callback::from(move |id: String| state.dispatch(RAction::GoToLogs(id)))
    };
    let on_history = {
        let state = state.clone();
        Callback::from(move |id: String| state.dispatch(RAction::GoToHistory(id)))
    };
    let on_flags = {
        let state = state.clone();
        Callback::from(move |id: String| state.dispatch(RAction::GoToFlags(id)))
    };
    let on_back = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(RAction::GoToList))
    };
    let on_edit = {
        let state = state.clone();
        Callback::from(move |id: String| state.dispatch(RAction::OpenEdit(id)))
    };
    let on_clone = {
        let state = state.clone();
        Callback::from(move |id: String| state.dispatch(RAction::GoToClone(id)))
    };
    let on_ask_delete = {
        let state = state.clone();
        Callback::from(move |(id, title): (String, String)| {
            state.dispatch(RAction::OpenConfirmDelete { id, title })
        })
    };
    let on_set_view = {
        let state = state.clone();
        Callback::from(move |view: RView| state.dispatch(RAction::SetView(view)))
    };
    let on_set_group_by = {
        let state = state.clone();
        Callback::from(move |by: RGroupBy| state.dispatch(RAction::SetGroupBy(by)))
    };
    let on_set_query = {
        let state = state.clone();
        Callback::from(move |q: String| state.dispatch(RAction::SetQuery(q)))
    };
    let on_set_status = {
        let state = state.clone();
        Callback::from(move |st: RoutineStatusFacet| state.dispatch(RAction::SetStatusFacet(st)))
    };
    let on_set_agent = {
        let state = state.clone();
        Callback::from(move |ag: AgentFacet| state.dispatch(RAction::SetAgentFacet(ag)))
    };
    let on_set_machine = {
        let state = state.clone();
        Callback::from(move |m: RoutineMachineFacet| state.dispatch(RAction::SetMachineFacet(m)))
    };
    let on_set_repository = {
        let state = state.clone();
        Callback::from(move |rp: RepositoryFacet| state.dispatch(RAction::SetRepositoryFacet(rp)))
    };
    let on_set_tag = {
        let state = state.clone();
        Callback::from(move |t: TagFacet| state.dispatch(RAction::SetTagFacet(t)))
    };
    let on_clear_filters = {
        let state = state.clone();
        Callback::from(move |_: ()| state.dispatch(RAction::ClearFilters))
    };

    let search_ref = use_node_ref();
    install_search_hotkey(search_ref.clone(), state.clone());

    let on_col_sort = {
        let state = state.clone();
        Callback::from(move |col: RCol| state.dispatch(RAction::SortByCol(col)))
    };

    // ── CRUD/API actions ──────────────────────────────────────────────────────
    let CrudHandlers {
        on_unlock_all,
        on_create,
        on_cleanup,
        on_trigger,
        on_toggle,
        on_save,
        on_confirm_delete,
    } = install_crud_handlers(state.clone(), toast.clone(), state.modal.clone());

    // ── Bulk selection ────────────────────────────────────────────────────────
    let BulkHandlers {
        on_select,
        on_select_all,
        on_clear_selection,
        on_bulk_enable,
        on_bulk_disable,
        on_bulk_delete,
        on_confirm_bulk_delete,
    } = install_bulk_handlers(state.clone(), toast.clone(), now.clone());

    let routines = state.routines.clone();
    let loading = state.loading;
    let page = state.page.clone();
    let modal = state.modal.clone();
    let lock_status = state.lock_status.clone();
    let view = state.view;
    let filter = state.filter.clone();
    let sort_col = state.sort_col;
    let sort_dir = state.sort_dir;
    let selected = state.selected.clone();
    let group_by = state.group_by;

    // Faceted filter + sort applied client-side.
    let now_val = *now;
    let window = Duration::seconds(DUE_SOON_WINDOW_SECS);
    let total_routines = routines.len();
    let agent_options = distinct_agents(&routines);
    let repository_options = distinct_repositories(&routines);
    let tag_options = distinct_tags(&routines);
    let mut machine_options = distinct_machines_r(&routines);
    // Always include the current machine so the default filter option is visible in the dropdown
    // even before any routine targets it.
    if let Some(cm) = &state.current_machine {
        if !machine_options.contains(cm) {
            machine_options.push(cm.clone());
            machine_options.sort();
        }
    }
    let filter_active = filter.is_active();
    let visible = {
        let filtered = filter_routines(&routines, &filter, now_val, window);
        sort_routines(filtered, sort_col, sort_dir, now_val)
    };
    let shown = visible.len();

    let edit_routine = match &modal {
        RModal::Edit(id) => routines.iter().find(|r| r.id == *id).cloned(),
        _ => None,
    };

    html! {
        <>
            {
                match page {
                    RPage::New => html! {
                        <RoutineForm editing={None} on_cancel={on_cancel} on_save={on_create} />
                    },
                    RPage::Clone(source) => {
                        let mut pre = *source;
                        pre.title = clone_title(&pre.title);
                        html! {
                            <RoutineForm editing={Some(pre)} on_cancel={on_cancel} on_save={on_create} />
                        }
                    },
                    RPage::Logs(id) => {
                        let title = routines.iter()
                            .find(|r| r.id == id)
                            .map(|r| r.title.clone())
                            .unwrap_or_default();
                        html! { <RoutineLogs id={id} title={title} on_back={on_back} /> }
                    },
                    RPage::History(id) => {
                        let title = routines.iter()
                            .find(|r| r.id == id)
                            .map(|r| r.title.clone())
                            .unwrap_or_default();
                        html! { <RoutineHistory id={id} title={title} on_back={on_back} /> }
                    },
                    RPage::Flags(id) => {
                        let title = routines.iter()
                            .find(|r| r.id == id)
                            .map(|r| r.title.clone())
                            .unwrap_or_default();
                        html! { <RoutineFlags id={id} title={title} on_back={on_back} /> }
                    },
                    RPage::List => html! {
                        <main>
                            <GlobalLockBanner status={lock_status} on_unlock={on_unlock_all} />
                            <RoutineStatsBar
                                routines={routines.clone()}
                                now={now_val}
                                active={filter.status}
                                on_status={on_set_status.clone()}
                            />
                            <div class="section-hd">
                                <div class="section-label">{"SCHEDULED ROUTINES"}</div>
                                <div class="section-acts">
                                    <RefreshControl
                                        interval={*interval}
                                        updated_at_ms={*updated_at}
                                        on_change={on_set_interval}
                                    />
                                    if view == RView::Table {
                                        <RoutineGroupBySelector
                                            group_by={group_by}
                                            on_change={on_set_group_by}
                                        />
                                    }
                                    <ViewToggle view={view} on_set_view={on_set_view} />
                                    <button class="btn btn-ghost btn-sm" onclick={on_cleanup}
                                        title="Reap finished, expired run workbenches now">{"CLEANUP NOW"}</button>
                                    <button class="btn btn-primary btn-sm" onclick={on_new}>{"+ NEW ROUTINE"}</button>
                                </div>
                            </div>
                            <FilterSortBar
                                filter={filter.clone()}
                                agents={agent_options}
                                machines={machine_options}
                                repositories={repository_options}
                                tags={tag_options}
                                shown={shown}
                                total={total_routines}
                                search_ref={search_ref.clone()}
                                on_query={on_set_query}
                                on_status={on_set_status}
                                on_agent={on_set_agent}
                                on_machine={on_set_machine}
                                on_repository={on_set_repository}
                                on_tag={on_set_tag}
                                on_clear={on_clear_filters.clone()}
                            />
                            <RoutineBulkBar
                                count={selected.len()}
                                on_enable={on_bulk_enable}
                                on_disable={on_bulk_disable}
                                on_delete={on_bulk_delete}
                                on_clear={on_clear_selection}
                            />
                            {
                                match view {
                                    RView::Table => html! {
                                        <RoutineTable
                                            routines={visible}
                                            loading={loading}
                                            filter_active={filter_active}
                                            now={now_val}
                                            selected={selected.clone()}
                                            on_select={on_select}
                                            on_select_all={on_select_all}
                                            sort_col={sort_col}
                                            sort_dir={sort_dir}
                                            group_by={group_by}
                                            on_sort={on_col_sort}
                                            on_edit={on_edit}
                                            on_clone={on_clone}
                                            on_delete={on_ask_delete}
                                            on_toggle={on_toggle}
                                            on_trigger={on_trigger}
                                            on_logs={on_logs}
                                            on_history={on_history}
                                            on_flags={on_flags}
                                            on_clear_filters={on_clear_filters}
                                        />
                                    },
                                    RView::Calendar => html! {
                                        <RoutineCalendar routines={visible} loading={loading} on_edit={Some(on_edit)} on_toast={Some(toast.clone())} />
                                    },
                                    RView::Day => {
                                        let items = visible.iter().filter(|r| r.enabled).map(|r| TimelineItem {
                                            id: Some(r.id.clone()),
                                            label: r.title.clone(),
                                            schedule: r.schedule.clone(),
                                            snoozed: is_routine_snoozed(r, now_val),
                                            flag_count: r.flag_count,
                                        }).collect::<Vec<_>>();
                                        html! { <DayTimeline items={items} loading={loading} on_click={Some(on_edit)} /> }
                                    },
                                }
                            }
                        </main>
                    },
                }
            }
            {
                match &modal {
                    RModal::Edit(_) => html! {
                        <RoutineForm editing={edit_routine} on_cancel={on_close.clone()} on_save={on_save} />
                    },
                    RModal::ConfirmDelete { id, title } => html! {
                        <ConfirmDelete
                            id={id.clone()}
                            title={title.clone()}
                            on_cancel={on_close}
                            on_confirm={on_confirm_delete}
                        />
                    },
                    RModal::ConfirmBulkDelete { count } => html! {
                        <RoutineBulkDeleteDialog
                            count={*count}
                            on_cancel={on_close}
                            on_confirm={on_confirm_bulk_delete}
                        />
                    },
                    RModal::None => html! {},
                }
            }
        </>
    }
}
