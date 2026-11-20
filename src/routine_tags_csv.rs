//! The tracked `tags.csv` sidecar: a routine's tags as one CSV record, plus the startup seed that
//! moves legacy `routine.toml` tags into it.

use std::path::Path;

use serde::Deserialize;

/// Legacy view of a `routine.toml` that still carried the `tags` key.
#[derive(Debug, Deserialize)]
struct LegacyRoutineTags {
    /// Legacy tag list, used only to seed `tags.csv` during directory migration.
    #[serde(default)]
    tags: Vec<String>,
}

/// Read the tags stored in a `tags.csv` sidecar.
///
/// Returns `None` when the file is absent or cannot be parsed, so the caller can fall back to the
/// legacy `routine.toml` key. An empty file yields `Some(vec![])`. Every field is trimmed and blank
/// fields are dropped.
pub(crate) fn read_routine_tags(path: &Path) -> Option<Vec<String>> {
    let bytes = std::fs::read(path).ok()?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(bytes.as_slice());
    let mut tags = Vec::new();
    for record in reader.records() {
        let record = match record {
            Ok(record) => record,
            Err(err) => {
                log::warn!(
                    "routine_tags_csv: ignoring malformed tags.csv {}: {err}",
                    path.display()
                );
                return None;
            }
        };
        for field in &record {
            let tag = field.trim();
            if !tag.is_empty() {
                tags.push(tag.to_string());
            }
        }
    }
    Some(tags)
}

/// Format `tags` as the `tags.csv` content: one CSV record holding every tag in order, ending with
/// `\n`. Tags containing a comma or quote are quoted so they round-trip. No tags yields an empty
/// string.
pub(crate) fn format_routine_tags(tags: &[String]) -> String {
    if tags.is_empty() {
        return String::new();
    }
    let mut writer = csv::WriterBuilder::new()
        .terminator(csv::Terminator::Any(b'\n'))
        .from_writer(Vec::new());
    writer.write_record(tags).ok();
    writer
        .into_inner()
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_default()
}

/// Seed a missing `tags.csv` from the legacy `tags` key of a `routine.toml` in `routine_dir`.
///
/// Does nothing when `tags.csv` already exists or the legacy list is absent or empty.
pub(crate) fn seed_tags_csv_from_legacy_toml(routine_dir: &Path) {
    let csv_path = routine_dir.join("tags.csv");
    if csv_path.exists() {
        return;
    }
    let Some(tags) = std::fs::read_to_string(routine_dir.join("routine.toml"))
        .ok()
        .and_then(|text| toml::from_str::<LegacyRoutineTags>(&text).ok())
        .map(|legacy| legacy.tags)
        .filter(|tags| !tags.is_empty())
    else {
        return;
    };
    if let Err(err) = std::fs::write(&csv_path, format_routine_tags(&tags)) {
        log::warn!(
            "migrate_routine_dirs: failed to seed {} from legacy routine.toml tags: {err}",
            csv_path.display()
        );
    }
}
