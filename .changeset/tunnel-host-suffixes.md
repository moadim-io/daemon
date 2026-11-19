---
"moadim": minor
---

Accept public dev-tunnel hostnames (for example Cloudflare Quick Tunnel `*.trycloudflare.com`) without a daemon restart. Name the tunnel domain once with `tunnel_host_suffixes` in `machine.local.toml`. The `Host` and mutation `Origin` checks still reject any other host.
