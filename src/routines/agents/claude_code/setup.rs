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
///
/// On Unix the seeding is a `python3` one-liner serialized by an `flock`; on Windows it is an inline
/// PowerShell snippet (run.ps1 is itself PowerShell) that edits `%USERPROFILE%\.claude.json`.
#[cfg(not(windows))]
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

/// Default `claude.toml` contents on Windows. The `setup` step is inline PowerShell that seeds the
/// same per-directory keys (`hasTrustDialogAccepted`, `disabledMcpjsonServers`) in
/// `%USERPROFILE%\.claude.json`, keyed by the workbench path `$wb` (in scope in `run.ps1`), so the
/// unattended session never blocks on the trust dialog or an MCP-server approval prompt. The
/// read-modify-write is committed via a temp file + atomic `Move-Item` so a concurrent reader never
/// sees a spliced, unparseable file.
#[cfg(windows)]
pub const CONFIG: &str = r#"command = "claude"
args = ["--permission-mode", "auto", "{prompt}"]
# Pre-seed per-directory state for the fresh workbench so claude launches unattended. Keyed per
# exact path in %USERPROFILE%\.claude.json (not inherited from parent dirs), seeded per run with
# $wb in scope:
#   - hasTrustDialogAccepted: skip the "Do you trust this folder?" dialog.
#   - disabledMcpjsonServers: skip the "New MCP server found" approval for project-scoped servers.
setup = '''
$claudeJson = Join-Path $env:USERPROFILE '.claude.json'
try {
  $cfg = if (Test-Path -LiteralPath $claudeJson) { Get-Content -Raw -LiteralPath $claudeJson | ConvertFrom-Json } else { New-Object psobject }
  if (-not ($cfg.PSObject.Properties.Name -contains 'projects')) { $cfg | Add-Member -NotePropertyName projects -NotePropertyValue (New-Object psobject) -Force }
  $entry = New-Object psobject
  $entry | Add-Member -NotePropertyName hasTrustDialogAccepted -NotePropertyValue $true -Force
  $entry | Add-Member -NotePropertyName hasCompletedProjectOnboarding -NotePropertyValue $true -Force
  $entry | Add-Member -NotePropertyName projectOnboardingSeenCount -NotePropertyValue 1 -Force
  $mcp = @()
  foreach ($f in @((Join-Path $env:USERPROFILE '.mcp.json'), (Join-Path $wb '.mcp.json'))) {
    if (Test-Path -LiteralPath $f) { $m = Get-Content -Raw -LiteralPath $f | ConvertFrom-Json; if ($m.mcpServers) { $mcp += $m.mcpServers.PSObject.Properties.Name } }
  }
  $entry | Add-Member -NotePropertyName disabledMcpjsonServers -NotePropertyValue (@($mcp | Sort-Object -Unique)) -Force
  $cfg.projects | Add-Member -NotePropertyName $wb -NotePropertyValue $entry -Force
  $tmp = "$claudeJson." + [System.Guid]::NewGuid().ToString('N') + '.tmp'
  $cfg | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $tmp -Encoding utf8
  Move-Item -LiteralPath $tmp -Destination $claudeJson -Force
} catch {}
'''
"#;
