---
"moadim": patch
---

feat(routines): `MOADIM_MAX_CONCURRENT_RUNS` now defaults to unlimited (`0`)

The global routine concurrency cap (#335) previously defaulted to `4` and rejected `0` as an
"off" value, always falling back to the default instead. That was inconsistent with
`MOADIM_MAX_WORKBENCH_DISK_BYTES`'s "0 means unbounded" convention elsewhere in the daemon.
`0` (or unset) now means no cap is enforced; set `MOADIM_MAX_CONCURRENT_RUNS` to a positive
number to opt into bounding how many routine agent sessions may run at once.
