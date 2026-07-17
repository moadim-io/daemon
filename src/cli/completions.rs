//! `moadim completions <shell>`: print a shell-completion script to stdout.
//!
//! [`crate::cli::parse`] hand-rolls its own argument parsing rather than using `clap` (see its
//! module docs), so there is no single `clap::Command` to hand `clap_complete` for the whole CLI.
//! [`build_cli`] below builds one anyway, purely as a completions source: it mirrors the
//! verb/flag surface documented in [`super::help_text`], and generating from it is far less code
//! (and far less likely to drift out of sync with reality) than hand-writing three shells' worth
//! of completion scripts by hand, which is what issue #307 originally proposed.

use std::io::Write;

use clap::{Arg, ArgAction, Command as ClapCommand, ValueEnum as _};
use clap_complete::Shell;

/// Build the synthetic `clap::Command` tree used only to generate shell completions. It is never
/// used to actually parse `std::env::args()` — [`super::parse`] still owns that.
pub(super) fn build_cli() -> ClapCommand {
    let json_flag = || Arg::new("json").long("json").action(ArgAction::SetTrue);
    let quiet_flag = || {
        Arg::new("quiet")
            .short('q')
            .long("quiet")
            .action(ArgAction::SetTrue)
    };
    let interactive_flag = || {
        Arg::new("interactive")
            .short('i')
            .long("interactive")
            .action(ArgAction::SetTrue)
    };

    ClapCommand::new("moadim")
        .about("routine scheduler with an MCP/REST API and a web control panel")
        .arg(interactive_flag().long_help("run in the foreground, attached to the terminal"))
        .arg(
            Arg::new("foreground")
                .short('f')
                .long("foreground")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("background")
                .short('b')
                .long("background")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("detach")
                .short('d')
                .long("detach")
                .action(ArgAction::SetTrue),
        )
        .arg(Arg::new("daemon").long("daemon").action(ArgAction::SetTrue))
        .subcommand(
            ClapCommand::new("restart")
                .arg(json_flag())
                .arg(quiet_flag())
                .arg(interactive_flag()),
        )
        .subcommand(ClapCommand::new("stop").arg(json_flag()).arg(quiet_flag()))
        .subcommand(
            ClapCommand::new("status")
                .arg(json_flag())
                .arg(Arg::new("wait").long("wait").num_args(0..=1)),
        )
        .subcommand(ClapCommand::new("cleanup").arg(json_flag()))
        .subcommand(ClapCommand::new("trigger").alias("run").arg(Arg::new("id")))
        .subcommand(ClapCommand::new("logs").arg(Arg::new("id")))
        .subcommand(ClapCommand::new("install"))
        .subcommand(ClapCommand::new("uninstall"))
        .subcommand(
            ClapCommand::new("machine")
                .subcommand(ClapCommand::new("show"))
                .subcommand(ClapCommand::new("set").arg(Arg::new("name")))
                .subcommand(ClapCommand::new("list")),
        )
        // `help` is not listed explicitly: `clap` auto-generates a `help` subcommand for any
        // `Command` that has subcommands, so adding one here would collide with it.
        .subcommand(ClapCommand::new("version"))
        .subcommand(
            ClapCommand::new("completions")
                .about("print a shell-completion script")
                .arg(
                    Arg::new("shell")
                        .value_parser(clap::value_parser!(Shell))
                        .required(true),
                ),
        )
        // Data-plane subcommands (`routines`, `agents`, `enable`, `disable`, `schedule`) are a
        // separate clap parser already (`crate::commands::DataCli`); listing their bare names
        // here (with no further flag detail) is enough for top-level tab-completion to find them.
        .subcommand(ClapCommand::new("routines").visible_alias("routine"))
        .subcommand(ClapCommand::new("schedule").visible_alias("sched"))
        .subcommand(ClapCommand::new("enable").arg(Arg::new("routine")))
        .subcommand(ClapCommand::new("disable").arg(Arg::new("routine")))
        .subcommand(ClapCommand::new("agents"))
}

/// Write the completion script for `shell_name` to `out`.
///
/// Returns `Err(())` when `shell_name` doesn't name a shell `clap_complete` supports (bash,
/// elvish, fish, powershell, zsh), leaving the caller to report the usage error.
pub(super) fn write_completions(shell_name: &str, out: &mut impl Write) -> Result<(), ()> {
    let shell = Shell::from_str(shell_name, false).map_err(|_ignored| ())?;
    let mut cmd = build_cli();
    clap_complete::generate(shell, &mut cmd, "moadim", out);
    Ok(())
}

/// Print a `completions`-specific usage error to stderr, matching [`super::print_usage_error`]'s
/// two-line shape.
fn print_completions_usage_error(detail: &str) {
    eprintln!("moadim: {detail}");
    eprintln!("Run `moadim help` for usage.");
}

/// `moadim completions <shell>`: print `shell_name`'s completion script to stdout, or a usage
/// error to stderr when `shell_name` is missing or unrecognized.
///
/// Returns the process exit code: `0` on success, [`super::EXIT_USAGE`] otherwise.
pub fn completions(shell_name: Option<&str>) -> i32 {
    let Some(name) = shell_name else {
        print_completions_usage_error(
            "completions requires a shell argument (bash, elvish, fish, powershell, zsh)",
        );
        return super::EXIT_USAGE;
    };
    let mut stdout = std::io::stdout();
    write_completions(name, &mut stdout).map_or_else(
        |()| {
            print_completions_usage_error(&format!(
                "unknown shell: {name} (expected bash, elvish, fish, powershell, or zsh)"
            ));
            super::EXIT_USAGE
        },
        |()| 0,
    )
}
