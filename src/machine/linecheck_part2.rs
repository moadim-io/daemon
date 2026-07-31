#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

/// `true` if `name` matches `pattern`, where `*` stands for any (possibly empty) run of characters
/// and every other character must match literally. Classic greedy two-pointer wildcard match with
/// backtracking to the most recent `*` on a mismatch.
pub(crate) fn glob_match(pattern: &str, name: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let name: Vec<char> = name.chars().collect();
    let (mut pi, mut ni) = (0, 0);
    let mut star: Option<usize> = None;
    let mut star_match = 0;
    while ni < name.len() {
        if pi < pattern.len() && pattern[pi] == name[ni] {
            pi += 1;
            ni += 1;
        } else if pi < pattern.len() && pattern[pi] == '*' {
            star = Some(pi);
            star_match = ni;
            pi += 1;
        } else if let Some(star_pi) = star {
            pi = star_pi + 1;
            star_match += 1;
            ni = star_match;
        } else {
            return false;
        }
    }
    while pattern.get(pi) == Some(&'*') {
        pi += 1;
    }
    pi == pattern.len()
}

/// Run the `moadim machine` CLI subcommand, returning the process exit code.
pub fn run(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        None | Some("show") => cmd_show(),
        Some("set") => {
            if let Some(name) = args.get(1) {
                cmd_set(name)
            } else {
                eprintln!("usage: moadim machine set <name>");
                2
            }
        }
        Some("list") => cmd_list(),
        Some(other) => {
            eprintln!("unknown machine subcommand {other:?}; expected show, set, or list");
            2
        }
    }
}

/// `moadim machine show` — print the resolved machine name and where it came from.
pub(crate) fn cmd_show() -> i32 {
    let (name, source) = resolve();
    println!("{name} (from {})", source.label());
    0
}

/// `moadim machine set <name>` — persist the machine identity.
pub(crate) fn cmd_set(name: &str) -> i32 {
    match set_machine(name) {
        Ok(()) => {
            println!("machine name set to {:?}", name.trim());
            0
        }
        Err(err) => {
            eprintln!("error: failed to set machine name: {err}");
            1
        }
    }
}

/// `moadim machine list` — print the distinct machine names referenced by routines/jobs.
pub(crate) fn cmd_list() -> i32 {
    let names = referenced_machines();
    if names.is_empty() {
        println!("no machines referenced by any routine");
    } else {
        for name in &names {
            println!("{name}");
        }
    }
    0
}
