---
"moadim": minor
---

Accept public dev-tunnel hostnames without a daemon restart. Name a tunnel domain you control once with `tunnel_host_suffixes` in `machine.local.toml`. A suffix needs two or more labels, and `*.` prefixes and trailing dots are ignored. The `Host` and mutation `Origin` checks still reject any other host. Avoid `trycloudflare.com` for anything but short-lived dev use, since every Quick Tunnel shares it.
