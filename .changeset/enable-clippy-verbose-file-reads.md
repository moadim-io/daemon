---
"moadim": patch
---

Enable `clippy::verbose_file_reads` to require `fs::read`/`fs::read_to_string` over manual `File::open` + `read_to_end`/`read_to_string`. The two sites that genuinely need the verbose form (reading a log's tail from a seeked offset, not the whole file) get a documented `#[allow]`.
