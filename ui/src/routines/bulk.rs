//! Bulk-selection action bar, bulk-delete confirmation, and the single-routine
//! delete confirmation dialog.

use yew::prelude::*;

// ─── Bulk action bar ──────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct RoutineBulkBarProps {
    pub count: usize,
    pub on_enable: Callback<()>,
    pub on_disable: Callback<()>,
    pub on_delete: Callback<()>,
    pub on_clear: Callback<()>,
}

/// Floating bulk-action toolbar. Hidden until at least one routine is selected.
///
/// Best-practice (Eleken UX guide, GitHub Actions): the bar appears in-context
/// as soon as a row is selected, shows the count, and offers primary actions
/// (enable/disable/delete) plus a clear affordance — no separate "actions" menu
/// needed.
#[function_component(RoutineBulkBar)]
pub fn routine_bulk_bar(props: &RoutineBulkBarProps) -> Html {
    if props.count == 0 {
        return html! {};
    }
    let mk = |cb: Callback<()>| Callback::from(move |_: MouseEvent| cb.emit(()));

    html! {
        <div class="bulk-bar">
            <span class="bulk-count">{ format!("{} SELECTED", props.count) }</span>
            <div class="bulk-acts">
                <button class="btn btn-ghost btn-sm" onclick={mk(props.on_enable.clone())}>{"ENABLE"}</button>
                <button class="btn btn-ghost btn-sm" onclick={mk(props.on_disable.clone())}>{"DISABLE"}</button>
                <button class="btn btn-danger btn-sm" onclick={mk(props.on_delete.clone())}>{"DELETE"}</button>
                <button class="btn btn-ghost btn-sm" onclick={mk(props.on_clear.clone())}>{"CLEAR"}</button>
            </div>
        </div>
    }
}

// ─── Bulk delete confirm dialog ───────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct RoutineBulkDeleteProps {
    pub count: usize,
    pub on_cancel: Callback<()>,
    pub on_confirm: Callback<()>,
}

#[function_component(RoutineBulkDeleteDialog)]
pub fn routine_bulk_delete_dialog(props: &RoutineBulkDeleteProps) -> Html {
    let on_cancel = {
        let cb = props.on_cancel.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };
    let on_confirm = {
        let cb = props.on_confirm.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };

    html! {
        <div class="overlay open">
            <div class="confirm-dialog">
                <div class="confirm-title">{"⚠ DELETE ROUTINES"}</div>
                <div class="confirm-msg">
                    { format!("Delete {} selected routine(s)? This cannot be undone.", props.count) }
                </div>
                <div class="confirm-acts">
                    <button class="btn btn-ghost btn-sm" onclick={on_cancel}>{"CANCEL"}</button>
                    <button class="btn btn-danger btn-sm" onclick={on_confirm}>{"DELETE"}</button>
                </div>
            </div>
        </div>
    }
}

// ─── Confirm delete ───────────────────────────────────────────────────────────

#[derive(Properties, PartialEq)]
pub struct ConfirmProps {
    pub id: String,
    pub title: String,
    pub on_cancel: Callback<()>,
    pub on_confirm: Callback<String>,
}

#[function_component(ConfirmDelete)]
pub fn confirm_delete(props: &ConfirmProps) -> Html {
    let on_cancel = {
        let cb = props.on_cancel.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };
    let on_confirm = {
        let cb = props.on_confirm.clone();
        let id = props.id.clone();
        Callback::from(move |_: MouseEvent| cb.emit(id.clone()))
    };

    html! {
        <div class="overlay open">
            <div class="confirm-dialog">
                <div class="confirm-title">{"⚠ DELETE ROUTINE"}</div>
                <div class="confirm-msg">
                    { format!("Delete the routine \"{}\"? This cannot be undone.", props.title) }
                </div>
                <div class="confirm-acts">
                    <button class="btn btn-ghost btn-sm" onclick={on_cancel}>{"CANCEL"}</button>
                    <button class="btn btn-danger btn-sm" onclick={on_confirm}>{"DELETE"}</button>
                </div>
            </div>
        </div>
    }
}
