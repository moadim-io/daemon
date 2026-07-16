---
"moadim": patch
---

fix(sync): keep a slow crontab sync from stalling the async runtime

`sync_routines_to_crontab` shells out to `crontab -l`/`crontab -` synchronously from async REST/MCP
request handlers. Run inline on the multi-thread runtime, a slow or hung `crontab` binary could tie
up a worker thread and stall unrelated in-flight requests, including `/health` (#360). It now runs
via `tokio::task::block_in_place` whenever a multi-thread runtime is present, so the runtime can hand
off that thread's other scheduled work first; unit tests (which call the function directly with no
runtime, or under `#[tokio::test]`'s single-thread default) are unaffected and continue to run inline.
