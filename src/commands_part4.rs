
/// Parse `args` (argv with the binary name stripped) and run the selected data subcommand against
/// the running server, returning the process exit code to surface.
///
/// On a clap parse error (bad flags, `--help`, `--version`) the formatted message is printed and the
/// matching code returned (`0` for help/version, `2` for a usage error), mirroring clap conventions
/// without aborting the process so the path stays unit-testable.
pub fn run(args: Vec<String>) -> i32 {
    match DataCli::try_parse_from(args) {
        Ok(cli) => dispatch(cli.command),
        Err(err) => {
            let _ = err.print();
            match err.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => 0,
                _ => 2,
            }
        }
    }
}

/// Route a parsed [`DataCommand`] to the matching REST call.
pub(crate) fn dispatch(command: DataCommand) -> i32 {
    match command {
        DataCommand::Routines(cmd) => dispatch_routine(*cmd),
        DataCommand::Schedule(ScheduleCmd::Trigger { id }) => request(
            "POST",
            &format!("{}/scheduled-trigger", routine_path(&id)),
            None,
        ),
        DataCommand::Enable { routine, json } => set_routine_enabled(&routine, true, json),
        DataCommand::Disable { routine, json } => set_routine_enabled(&routine, false, json),
        DataCommand::Agents => request("GET", "/api/v1/agents", None),
    }
}
