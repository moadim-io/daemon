# moadim vs. the alternatives

Feature comparison: scheduling AI agents on a recurring basis.
moadim's niche is **routines** — a prompt + schedule + coding agent that runs
unattended in a throwaway workbench, locally, with no cloud dependency.

## At a glance

| Capability                       | **moadim**            | cron / systemd timers | GitHub Actions (cron) | Claude Code scheduling¹ | Other agent runners² |
| -------------------------------- | --------------------- | --------------------- | --------------------- | ----------------------- | -------------------- |
| Schedule a **script**            | ❌ (routines only)    | ✅                    | ✅                    | ⚠️ via shell step       | ✅                   |
| Schedule an **AI agent + prompt**| ✅ routines           | ❌ (DIY wrapper)      | ⚠️ DIY in a workflow  | ✅                      | ✅                   |
| Runs **locally** / offline       | ✅                    | ✅                    | ❌ cloud              | ⚠️ varies               | ⚠️ varies            |
| **Agent-agnostic** (swap CLI)    | ✅ claude/codex/hermes| n/a                   | ✅ any                | ❌ Claude only          | ❌ usually 1 vendor  |
| **Per-run isolation** (workbench)| ✅ throwaway + reaped | ❌                    | ✅ fresh runner       | ⚠️ varies               | ⚠️ varies            |
| **Unattended auth** pre-seed     | ✅ (claude trust/MCP) | n/a                   | ⚠️ secrets/tokens     | ✅                      | ⚠️ varies            |
| Config **git-trackable**         | ✅ TOML + prompt files| ⚠️ crontab file       | ✅ YAML in repo       | ⚠️ varies               | ⚠️ varies            |
| **3 interfaces, one port**       | ✅ UI + REST + MCP    | ❌ CLI only           | ❌ web/API            | ⚠️ CLI/web              | ⚠️ varies            |
| **MCP** native (agents self-schedule) | ✅               | ❌                    | ❌                    | ⚠️                      | ⚠️                   |
| Calendar feed (`.ics`)           | ✅                    | ❌                    | ❌                    | ❌                      | ❌                   |
| Multi-machine targeting          | ✅ `machines` list    | ❌ per-host           | n/a (cloud)           | ⚠️                      | ⚠️                   |
| Run-now / trigger off-schedule   | ✅                    | ❌                    | ✅ manual dispatch    | ⚠️                      | ✅                   |
| Per-run **timeout** + **TTL**    | ✅ max_runtime/ttl    | ❌                    | ✅ job timeout        | ⚠️                      | ✅                   |
| Cost                             | free / self-host      | free                  | minutes-billed cloud  | subscription/usage      | varies               |
| Setup weight                     | one binary            | none                  | repo + cloud          | account                 | service/infra        |

✅ first-class  ⚠️ partial / DIY  ❌ not supported

¹ Claude Code routines / cloud agents / `/schedule` skill. ² Codex/Hermes
standalone, n8n, Temporal, Airflow, etc.

## Where each wins

- **cron / systemd timers** — bedrock for scripts. Zero deps, everywhere. No
  agent concept, no isolation, no API, no UI. moadim *sits on top* (syncs to the
  OS crontab) rather than replacing it.
- **GitHub Actions (cron)** — best when work is already CI-shaped and cloud is
  fine: fresh runners, secrets, repo-tracked YAML. Weak for local/offline,
  interactive agents, and sub-hour reliability; minute-billed.
- **Claude Code scheduling** — tight Claude integration, managed auth. Locked to
  Claude; not agent-agnostic; no unified local UI+REST+MCP surface; no `.ics`.
- **Other agent runners (n8n / Temporal / Airflow / Codex-Hermes standalone)** —
  powerful for complex DAGs/durable workflows, but heavier infra and usually
  single-vendor or not agent-prompt-native.

## Where moadim wins

- **Purpose-built for agent routines** — a prompt + schedule + coding agent,
  synced to the OS crontab, on the same daemon and port as the REST/MCP/UI surface.
- **Agent-agnostic** — `claude`, `codex`, `hermes` built in; any CLI via a
  `<name>.toml`. Swap the agent without touching the schedule.
- **Local-first, self-hosted, free** — no cloud, no per-minute billing, runs
  offline against the host crontab.
- **Three interfaces, one port** — UI + REST + MCP with no logic duplication. MCP
  means an agent can read, schedule, and re-fire its **own** routines.
- **Unattended by design** — per-run throwaway workbench (reaped on a 5-minute sweep), tmux
  session, pre-seeded Claude trust/MCP approvals so the session never blocks.
- **Ops niceties** — `.ics` feed of fire times, `machines` targeting, run-now
  trigger, `max_runtime_secs` kill cap, `ttl_secs` retention.

## When NOT to pick moadim

- You need cloud-hosted execution with no always-on local machine → GitHub
  Actions / Claude cloud agents.
- You need durable, multi-step DAG orchestration with retries/backfill →
  Temporal / Airflow.
- You only ever run plain scripts and want zero new software → raw cron.
