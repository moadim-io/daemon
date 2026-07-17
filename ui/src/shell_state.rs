//! [`ShellState`]/[`ShellAction`]: the reducer backing [`crate::Shell`] and its dialogs (health
//! status, toasts, and which shell-level overlay is open). Split out of `main.rs` to keep that
//! file under the workspace's 500-line-per-file convention (see `linecheck` in
//! `.github/workflows/lint.yml`) once `missing_docs` required documenting every field/variant here.

use yew::prelude::*;

use crate::{apply_theme, save_theme_light, Health, Toast, ToastKind};

/// Reducible state shared by [`crate::Shell`] and its dialogs: health status, toasts, and which
/// shell-level overlay (shutdown/palette/rename-machine) is currently open.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ShellState {
    /// Last health payload fetched from `GET /api/v1/health`.
    pub health: Health,
    /// Whether the last health fetch succeeded.
    pub health_ok: bool,
    /// Toasts currently queued for display in the toast stack.
    pub toasts: Vec<Toast>,
    /// Monotonically increasing id assigned to the next toast added.
    pub next_toast: u32,
    /// Whether the shutdown confirmation dialog is open.
    pub show_shutdown: bool,
    /// Whether the command palette is open.
    pub show_palette: bool,
    /// `true` when the light theme is active; persisted to localStorage.
    pub show_theme_light: bool,
    /// Resolved name of this machine, fetched from `GET /api/v1/machine` on mount.
    pub machine_name: Option<String>,
    /// Whether the rename-machine dialog is open.
    pub show_rename_machine: bool,
}

/// Actions dispatched against [`ShellState`] via its [`Reducible`] impl.
pub enum ShellAction {
    /// A health poll completed; carries the new payload and whether it succeeded.
    HealthLoaded {
        /// The fetched health payload.
        health: Health,
        /// Whether the fetch succeeded.
        ok: bool,
    },
    /// Queue a new toast for display.
    AddToast {
        /// The toast's message text.
        msg: String,
        /// The toast's visual style (success/error/info).
        kind: ToastKind,
    },
    /// Open the shutdown confirmation dialog.
    OpenShutdown,
    /// Close the shutdown confirmation dialog.
    CloseShutdown,
    /// Toggle the command palette open/closed.
    TogglePalette,
    /// Close the command palette.
    ClosePalette,
    /// Flip the active theme and persist the new choice.
    ToggleTheme,
    /// The machine name was fetched from `GET /api/v1/machine`.
    MachineName {
        /// The resolved machine name.
        name: String,
    },
    /// Open the rename-machine dialog.
    OpenRenameMachine,
    /// Close the rename-machine dialog.
    CloseRenameMachine,
}

impl Reducible for ShellState {
    type Action = ShellAction;

    fn reduce(self: std::rc::Rc<Self>, action: Self::Action) -> std::rc::Rc<Self> {
        let mut s = (*self).clone();
        match action {
            ShellAction::HealthLoaded { health, ok } => {
                s.health = health;
                s.health_ok = ok;
            }
            ShellAction::AddToast { msg, kind } => {
                let id = s.next_toast;
                s.next_toast += 1;
                s.toasts.push(Toast {
                    id,
                    msg: AttrValue::from(msg),
                    kind,
                });
            }
            ShellAction::OpenShutdown => s.show_shutdown = true,
            ShellAction::CloseShutdown => s.show_shutdown = false,
            ShellAction::TogglePalette => s.show_palette = !s.show_palette,
            ShellAction::ClosePalette => s.show_palette = false,
            ShellAction::ToggleTheme => {
                s.show_theme_light = !s.show_theme_light;
                save_theme_light(s.show_theme_light);
                apply_theme(s.show_theme_light);
            }
            ShellAction::MachineName { name } => s.machine_name = Some(name),
            ShellAction::OpenRenameMachine => s.show_rename_machine = true,
            ShellAction::CloseRenameMachine => s.show_rename_machine = false,
        }
        s.into()
    }
}
