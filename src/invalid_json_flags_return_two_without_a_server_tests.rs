
#[test]
fn invalid_json_flags_return_two_without_a_server() {
    // Body builders reject malformed JSON before any request is sent.
    assert_eq!(
        run(argv(&[
            "routines",
            "create",
            "--schedule",
            "* * * * *",
            "--title",
            "t",
            "--agent",
            "a",
            "--prompt",
            "p",
            "--repositories",
            "{bad",
        ])),
        2
    );
    assert_eq!(
        run(argv(&[
            "routines",
            "replace",
            "id",
            "--schedule",
            "* * * * *",
            "--title",
            "t",
            "--agent",
            "a",
            "--prompt",
            "p",
            "--repositories",
            "{bad",
        ])),
        2
    );
    assert_eq!(
        run(argv(&[
            "routines",
            "update",
            "id",
            "--repositories",
            "{bad"
        ])),
        2
    );
    // Malformed --machines JSON is rejected on the routine update path too.
    assert_eq!(
        run(argv(&["routines", "update", "id", "--machines", "{bad"])),
        2
    );
}

// ─── End-to-end dispatch against a fake server ───────────────────────────────

// ─── enable / disable ────────────────────────────────────────────────────────

// ─── Body-builder unit tests ─────────────────────────────────────────────────

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "every_subcommand_succeeds_against_a_2xx_server_tests.rs"]
mod every_subcommand_succeeds_against_a_2xx_server_tests;

#[allow(
    clippy::missing_docs_in_private_items,
    reason = "split-out module keeps the file under the linecheck limit"
)]
#[path = "routine_body_serializes_all_fields_tests.rs"]
mod routine_body_serializes_all_fields_tests;
