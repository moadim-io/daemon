---
"moadim": minor
---

### Added

- **Structured JSON logging.** Set `MOADIM_LOG_FORMAT=json` to switch `daemon.log`
  (and foreground stdout) from `env_logger`'s human-readable format to one JSON
  object per line (`ts`, `level`, `target`, `msg`), so a `launchd`/`systemd`-run
  daemon can ship its log into an aggregator (Loki, ELK, Vector, CloudWatch)
  without regex-scraping free-form text. Opt-in — the variable unset keeps the
  current text format byte-for-byte, and `RUST_LOG` level filtering is unchanged
  in both formats. (#416)
