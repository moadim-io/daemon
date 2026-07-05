---
"moadim": patch
---

feat(ui): show open-flag badge on upcoming-runs rows

The UPCOMING RUNS table now shows a small "⚑ N" badge next to the name
of any routine that has open flags, so operators can see at a glance
which about-to-fire routines still need flag review without navigating
to the NEEDS ATTENTION panel. Two new tests verify the flag count is
correctly propagated from SchedSource to UpcomingRun.
