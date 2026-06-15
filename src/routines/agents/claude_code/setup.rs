//! Built-in default agent config for Claude Code.

/// Registry key for this agent; also the config filename stem (`claude.toml`).
pub const NAME: &str = "claude";

/// Default `claude.toml` contents, written on startup when the file is absent.
///
/// Launches Claude Code interactively in the workbench and passes the prompt as a process argument
/// (`{prompt}`). The `setup` step pre-seeds per-directory state in `~/.claude.json`
/// — `hasTrustDialogAccepted` and `disabledMcpjsonServers` — so the unattended session never blocks
/// on the workspace-trust dialog or a project MCP-server approval prompt. Both are keyed by exact
/// path (not inherited from parent dirs), so they must be seeded for each fresh workbench.
pub const CONFIG: &str = r#"command = "claude"
args = ["--permission-mode", "auto", "{prompt}"]
# Pre-seed per-directory state for the fresh workbench so interactive claude launches unattended,
# with no blocking prompts. Both are stored per exact path in ~/.claude.json (not inherited from
# parent dirs), so they must be seeded per run. Runs with $WB in scope before launch:
#   - hasTrustDialogAccepted: skip the "Do you trust this folder?" dialog.
#   - disabledMcpjsonServers: skip the "New MCP server found" approval for any project-scoped
#     servers claude would discover from ancestor .mcp.json files (e.g. ~/.mcp.json).
# The read-modify-write is serialized by an flock on ~/.claude.json.lock and committed via a
# temp file + os.replace (atomic rename). ~/.claude.json is shared with the live claude process,
# so a plain open("w") truncate-then-write could interleave with a concurrent writer and leave a
# spliced, unparseable file; the atomic replace guarantees readers always see one complete JSON.
setup = '''python3 -c 'import json,os,sys,tempfile,fcntl; home=os.path.expanduser("~"); p=home+"/.claude.json"; lf=open(p+".lock","w"); fcntl.flock(lf,fcntl.LOCK_EX); d=json.load(open(p)) if os.path.exists(p) else {}; wb=sys.argv[1]; e=d.setdefault("projects",{}).setdefault(wb,{}); e.update({"hasTrustDialogAccepted":True,"hasCompletedProjectOnboarding":True}); e.setdefault("projectOnboardingSeenCount",1); g=lambda f: list(json.load(open(f)).get("mcpServers",{}).keys()) if os.path.exists(f) else []; e["disabledMcpjsonServers"]=sorted(set(e.get("disabledMcpjsonServers",[]))|set(g(home+"/.mcp.json"))|set(g(wb+"/.mcp.json"))); fd,tmp=tempfile.mkstemp(dir=home,prefix=".claude.json.",suffix=".tmp"); os.write(fd,json.dumps(d).encode()); os.close(fd); os.replace(tmp,p); fcntl.flock(lf,fcntl.LOCK_UN)' "$WB"'''
"#;
