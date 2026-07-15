//! The `moadim` dashboard: a Yew/WASM single-page app served by the daemon's HTTP server. Renders
//! the routines table, calendar heatmap, and settings pages against the `/api/v1` REST API, and
//! hosts the shell chrome (nav, health indicator, toasts, command palette) shared across all of
//! them.

use gloo_timers::future::TimeoutFuture;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;

mod command_palette;
mod command_palette_match;
mod cron_utils;
mod day_timeline;
mod header;
mod health;
mod log_viewer;
mod machines;
mod overview;
mod overview_attention;
mod overview_recent_runs;
mod overview_stats;
mod overview_upcoming;
mod refresh;
mod routines;
mod schedule;
mod schedule_heatmap;
mod schedule_heatmap_grid;
mod settings;
mod shell_dialogs;
mod shell_state;
use command_palette::CommandPalette;
pub(crate) use cron_utils::{abstime, describe_cron_live, parse_cron, reltime};
use header::Header;
pub(crate) use health::{Health, Toast, ToastKind};
use overview::OverviewPage;
use routines::RoutinesPage;
use schedule_heatmap::HeatmapPage;
use settings::SettingsPage;
use shell_dialogs::{
    api_get_machine, api_put_machine, api_shutdown, poll_health, RenameMachineDialog,
    ShutdownDialog, ToastStack,
};
pub(crate) use shell_state::{ShellAction, ShellState};

// ─── Theme ────────────────────────────────────────────────────────────────────

/// localStorage key for the theme preference.
pub(crate) const THEME_KEY: &str = "moadim.theme";

/// Read the persisted theme from localStorage. Returns `true` for light theme.
pub(crate) fn load_theme_light() -> bool {
    web_sys::window()
        .and_then(|win| win.local_storage().ok().flatten())
        .and_then(|store| store.get_item(THEME_KEY).ok().flatten())
        .is_some_and(|val| val == "light")
}

/// Persist the theme choice to localStorage (best-effort; ignores storage errors).
pub(crate) fn save_theme_light(light: bool) {
    if let Some(store) = web_sys::window().and_then(|win| win.local_storage().ok().flatten()) {
        let _ = store.set_item(THEME_KEY, if light { "light" } else { "dark" });
    }
}

/// Apply or remove the `theme-light` CSS class from `<html>`.
pub(crate) fn apply_theme(light: bool) {
    if let Some(root) = web_sys::window()
        .and_then(|win| win.document())
        .and_then(|doc| doc.document_element())
    {
        let list = root.class_list();
        if light {
            let _ = list.add_1("theme-light");
        } else {
            let _ = list.remove_1("theme-light");
        }
    }
}

// ─── Routes ───────────────────────────────────────────────────────────────────

/// Top-level pages, served at the root path space: `Routines` at `/routines` and `Heatmap` at
/// `/heatmap`. The REST API is namespaced under `/api/v1`, so these UI paths never collide with it.
/// The server returns the same self-contained HTML for any unmatched path (SPA fallback), letting
/// these deep links and refreshes load the app so the router can resolve the path.
#[derive(Clone, Routable, PartialEq, Eq)]
pub enum Route {
    /// The overview page (recent runs, upcoming runs, at-a-glance stats).
    #[at("/")]
    Home,
    /// The routines table page.
    #[at("/routines")]
    Routines,
    /// The calendar heatmap page.
    #[at("/heatmap")]
    Heatmap,
    /// The settings page.
    #[at("/settings")]
    Settings,
    /// Fallback for any path that doesn't match a route above.
    #[not_found]
    #[at("/404")]
    NotFound,
}

// ─── Root ─────────────────────────────────────────────────────────────────────

/// The app root: sets up the router and mounts [`Shell`] as its only route-independent child.
#[function_component(App)]
pub fn app() -> Html {
    html! {
        <BrowserRouter>
            <Shell />
        </BrowserRouter>
    }
}

/// Persistent chrome around the routed pages: header (health + STOP), nav tabs, the routed page,
/// the global shutdown dialog, and the toast stack. Lives inside the router so nav `Link`s work.
#[function_component(Shell)]
pub fn shell() -> Html {
    let state = use_reducer(|| ShellState {
        show_theme_light: load_theme_light(),
        ..ShellState::default()
    });

    // Apply the initial theme class from persisted preference.
    {
        let light = state.show_theme_light;
        use_effect_with((), move |()| {
            apply_theme(light);
        });
    }

    // Initial health poll + machine name fetch on mount.
    {
        let state = state.clone();
        use_effect_with((), move |()| {
            let state2 = state.clone();
            spawn_local(async move { poll_health(state).await });
            spawn_local(async move {
                if let Ok(name) = api_get_machine().await {
                    state2.dispatch(ShellAction::MachineName { name });
                }
            });
        });
    }

    // Health poll loop every 30 s.
    {
        let state = state.clone();
        use_effect_with((), move |()| {
            spawn_local(async move {
                loop {
                    TimeoutFuture::new(30_000).await;
                    poll_health(state.clone()).await;
                }
            });
        });
    }

    // Global ⌘K / Ctrl-K listener that toggles the command palette from any
    // page, and Escape to dismiss whichever shell-level dialog is open.
    // Registered once on mount and torn down on unmount.
    {
        let state = state.clone();
        use_effect_with((), move |()| {
            let on_key =
                Closure::<dyn Fn(KeyboardEvent)>::wrap(Box::new(move |event: KeyboardEvent| {
                    if (event.meta_key() || event.ctrl_key())
                        && event.key().eq_ignore_ascii_case("k")
                    {
                        event.prevent_default();
                        state.dispatch(ShellAction::TogglePalette);
                    } else if event.key() == "Escape" {
                        if state.show_shutdown {
                            state.dispatch(ShellAction::CloseShutdown);
                        } else if state.show_rename_machine {
                            state.dispatch(ShellAction::CloseRenameMachine);
                        }
                    }
                }));
            let window = web_sys::window().expect("window exists");
            window
                .add_event_listener_with_callback("keydown", on_key.as_ref().unchecked_ref())
                .expect("keydown listener attaches");
            move || {
                if let Some(window) = web_sys::window() {
                    let _ = window.remove_event_listener_with_callback(
                        "keydown",
                        on_key.as_ref().unchecked_ref(),
                    );
                }
                drop(on_key);
            }
        });
    }

    let on_toast = {
        let state = state.clone();
        Callback::from(move |(msg, kind): (String, ToastKind)| {
            state.dispatch(ShellAction::AddToast { msg, kind });
        })
    };

    let on_close_palette = {
        let state = state.clone();
        Callback::from(move |(): ()| state.dispatch(ShellAction::ClosePalette))
    };

    let on_open_palette = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| state.dispatch(ShellAction::TogglePalette))
    };

    // Palette "Refresh" / "Stop Server" actions mirror the header buttons but
    // take the `()` payload the palette emits.
    let on_palette_refresh = {
        let state = state.clone();
        Callback::from(move |(): ()| {
            let state = state.clone();
            spawn_local(async move { poll_health(state).await });
        })
    };
    let on_palette_stop = {
        let state = state.clone();
        Callback::from(move |(): ()| state.dispatch(ShellAction::OpenShutdown))
    };
    let on_palette_toggle_theme = {
        let state = state.clone();
        Callback::from(move |(): ()| state.dispatch(ShellAction::ToggleTheme))
    };

    let on_refresh = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| {
            let state = state.clone();
            spawn_local(async move { poll_health(state).await });
        })
    };

    // STOP only opens a confirmation dialog; the server is asked to stop on confirm.
    let on_stop = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| state.dispatch(ShellAction::OpenShutdown))
    };

    let on_close_shutdown = {
        let state = state.clone();
        Callback::from(move |(): ()| state.dispatch(ShellAction::CloseShutdown))
    };

    let on_confirm_shutdown = {
        let state = state.clone();
        Callback::from(move |(): ()| {
            let state = state.clone();
            state.dispatch(ShellAction::CloseShutdown);
            spawn_local(async move {
                match api_shutdown().await {
                    Ok(()) => {
                        state.dispatch(ShellAction::HealthLoaded {
                            health: Health {
                                status: "stopping".into(),
                                running: false,
                                uptime_secs: None,
                                version: None,
                                git_sha: None,
                                dependencies: None,
                            },
                            ok: false,
                        });
                        state.dispatch(ShellAction::AddToast {
                            msg: "Server stopping…".into(),
                            kind: ToastKind::Ok,
                        });
                    }
                    Err(e) => state.dispatch(ShellAction::AddToast {
                        msg: format!("Stop failed: {e}"),
                        kind: ToastKind::Err,
                    }),
                }
            });
        })
    };

    let switch = {
        let on_toast = on_toast.clone();
        Callback::from(move |route: Route| match route {
            Route::Home => html! { <OverviewPage on_toast={on_toast.clone()} /> },
            Route::Routines => html! { <RoutinesPage on_toast={on_toast.clone()} /> },
            Route::Heatmap => html! { <HeatmapPage /> },
            Route::Settings => html! { <SettingsPage on_toast={on_toast.clone()} /> },
            Route::NotFound => html! { <Redirect<Route> to={Route::Home} /> },
        })
    };

    let health = state.health.clone();
    let health_ok = state.health_ok;
    let toasts = state.toasts.clone();
    let show_shutdown = state.show_shutdown;
    let show_palette = state.show_palette;
    let show_theme_light = state.show_theme_light;
    let machine_name = state.machine_name.clone();
    let show_rename_machine = state.show_rename_machine;
    let on_theme = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| state.dispatch(ShellAction::ToggleTheme))
    };

    let on_open_rename_machine = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| state.dispatch(ShellAction::OpenRenameMachine))
    };
    let on_close_rename_machine = {
        let state = state.clone();
        Callback::from(move |(): ()| state.dispatch(ShellAction::CloseRenameMachine))
    };
    let on_confirm_rename_machine = {
        let state = state.clone();
        Callback::from(
            move |(name, on_done): (String, Callback<Result<(), String>>)| {
                let state = state.clone();
                spawn_local(async move {
                    match api_put_machine(&name).await {
                        Ok(new_name) => {
                            state.dispatch(ShellAction::MachineName {
                                name: new_name.clone(),
                            });
                            state.dispatch(ShellAction::CloseRenameMachine);
                            state.dispatch(ShellAction::AddToast {
                                msg: format!("Machine renamed to \"{new_name}\""),
                                kind: ToastKind::Ok,
                            });
                            on_done.emit(Ok(()));
                        }
                        Err(e) => {
                            state.dispatch(ShellAction::AddToast {
                                msg: format!("Rename failed: {e}"),
                                kind: ToastKind::Err,
                            });
                            on_done.emit(Err(e));
                        }
                    }
                });
            },
        )
    };

    html! {
        <>
            <Header health={health} ok={health_ok} light={show_theme_light}
                machine_name={machine_name.clone()}
                on_refresh={on_refresh} on_stop={on_stop} on_palette={on_open_palette}
                on_theme={on_theme} on_rename_machine={on_open_rename_machine} />
            <Nav />
            <Switch<Route> render={switch} />
            <CommandPalette
                open={show_palette}
                on_close={on_close_palette}
                on_refresh={on_palette_refresh}
                on_stop={on_palette_stop}
                on_toggle_theme={on_palette_toggle_theme}
            />
            {
                if show_rename_machine {
                    html! {
                        <RenameMachineDialog
                            current={machine_name.unwrap_or_default()}
                            on_cancel={on_close_rename_machine}
                            on_confirm={on_confirm_rename_machine}
                        />
                    }
                } else {
                    html! {}
                }
            }
            {
                if show_shutdown {
                    html! {
                        <ShutdownDialog
                            on_cancel={on_close_shutdown}
                            on_confirm={on_confirm_shutdown}
                        />
                    }
                } else {
                    html! {}
                }
            }
            <ToastStack toasts={toasts} />
        </>
    }
}

// ─── Nav ──────────────────────────────────────────────────────────────────────

/// The top-level tab bar, highlighting the tab matching the current route.
#[function_component(Nav)]
pub fn nav() -> Html {
    let route = use_route::<Route>().unwrap_or(Route::Home);
    let cls = |target: &Route| {
        if route == *target {
            "tab-btn active"
        } else {
            "tab-btn"
        }
    };
    html! {
        <nav class="tabs">
            <Link<Route> classes={classes!(cls(&Route::Home))} to={Route::Home}>
                { "OVERVIEW" }
            </Link<Route>>
            <Link<Route> classes={classes!(cls(&Route::Routines))} to={Route::Routines}>
                { "ROUTINES" }
            </Link<Route>>
            <Link<Route> classes={classes!(cls(&Route::Heatmap))} to={Route::Heatmap}>
                { "HEATMAP" }
            </Link<Route>>
            <Link<Route> classes={classes!(cls(&Route::Settings))} to={Route::Settings}>
                { "SETTINGS" }
            </Link<Route>>
        </nav>
    }
}

fn main() {
    console_log::init_with_level(log::Level::Info).unwrap_or_default();
    yew::Renderer::<App>::new().render();
}
