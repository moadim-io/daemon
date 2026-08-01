# Security Policy

## Supported versions

`moadim` ships from a single release line (no maintained LTS branches). Security
fixes land on the latest published release; please upgrade to the newest version
before reporting.

| Version         | Supported          |
| --------------- | ------------------ |
| Latest release  | :white_check_mark: |
| Older releases  | :x:                |

## Reporting a vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.** A
public issue discloses the problem to everyone before a fix exists.

Instead, report privately through GitHub's
[Private Vulnerability Reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing-information-about-vulnerabilities/privately-reporting-a-security-vulnerability):
open the repository's **Security** tab and choose **Report a vulnerability**.
This opens a private advisory visible only to the maintainers.

Please include:

- the affected version (`moadim --version`) and platform,
- a description of the issue and its impact,
- steps to reproduce or a proof of concept, and
- any suggested remediation, if known.

## Response expectations

This is a small, volunteer-maintained project. We aim to acknowledge a report
within **7 days** and to keep you updated as we triage and work on a fix.
Coordinated disclosure timing will be agreed with the reporter once the impact
is understood.

## Scope and threat model

`moadim` is a local HTTP daemon that holds the operator's agent credentials and
can schedule and trigger real agent runs. A few things are **by design** and
therefore out of scope as vulnerabilities:

- **Loopback by default, token for remote use.** The daemon binds to
  `127.0.0.1` by default. With no `MOADIM_API_TOKEN`, REST/MCP are intended for
  same-host loopback use. Setting `MOADIM_API_TOKEN=<secret>` enables
  shared-secret authentication for REST/MCP (`Authorization: Bearer ***` or
  `X-Moadim-Token: <secret>`). A non-loopback `MOADIM_BIND_ADDR` is refused
  unless either that token is configured, or the operator explicitly sets
  `MOADIM_ALLOW_REMOTE=1` to accept unauthenticated network exposure. Treat an
  unauthenticated non-loopback daemon as RCE-equivalent: anyone who can reach it
  can create/trigger routines that launch agents with the operator's
  credentials.

In scope, and very much worth reporting: memory-safety issues, authentication
or authorization bypasses within the intended loopback model, command or path
injection, crontab/launchd manipulation beyond a routine's own entries, and
similar flaws that violate the daemon's documented behavior.
