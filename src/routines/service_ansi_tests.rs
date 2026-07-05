#![allow(
    clippy::missing_docs_in_private_items,
    reason = "test helpers and fixtures do not need doc comments"
)]

use super::*;

#[test]
fn strip_ansi_noise_leaves_plain_text_untouched() {
    assert_eq!(
        strip_ansi_noise("plain log line\nsecond line\n"),
        "plain log line\nsecond line\n"
    );
}

#[test]
fn strip_ansi_noise_removes_csi_color_codes() {
    assert_eq!(strip_ansi_noise("\u{1B}[31mred\u{1B}[0m\n"), "red\n");
}

#[test]
fn strip_ansi_noise_removes_osc_sequence_terminated_by_bel() {
    assert_eq!(
        strip_ansi_noise("\u{1B}]0;window title\u{7}after\n"),
        "after\n"
    );
}

#[test]
fn strip_ansi_noise_removes_osc_sequence_terminated_by_escape_backslash() {
    assert_eq!(
        strip_ansi_noise("\u{1B}]0;window title\u{1B}\\after\n"),
        "after\n"
    );
}

#[test]
fn strip_ansi_noise_drops_bare_two_byte_escape() {
    // `ESC c` is a full terminal reset with no CSI/OSC bracket.
    assert_eq!(strip_ansi_noise("before\u{1B}cafter\n"), "beforeafter\n");
}

#[test]
fn strip_ansi_noise_drops_trailing_lone_escape() {
    assert_eq!(strip_ansi_noise("before\u{1B}"), "before");
}

#[test]
fn strip_ansi_noise_collapses_carriage_return_redraws() {
    assert_eq!(
        strip_ansi_noise("progress: 10%\rprogress: 100%\ndone\n"),
        "progress: 100%\ndone\n"
    );
}

#[test]
fn strip_ansi_noise_handles_combined_escape_and_redraw_noise() {
    assert_eq!(
        strip_ansi_noise("\u{1B}[2K\u{1B}[1Gspin .\rspin ..\rspin ...\ndone\u{1B}[0m\n"),
        "spin ...\ndone\n"
    );
}

#[test]
fn read_log_tail_strips_ansi_noise_from_a_whole_file_read() {
    let dir = std::env::temp_dir().join(format!("moadim-logtail-ansi-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("agent.log");
    std::fs::write(&path, "\u{1B}[31mred\u{1B}[0m line\rreal line\n").unwrap();

    assert_eq!(read_log_tail(&path).unwrap(), "real line\n");
    let _ = std::fs::remove_dir_all(&dir);
}
