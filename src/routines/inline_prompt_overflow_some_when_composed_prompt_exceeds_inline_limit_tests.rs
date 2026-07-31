#![allow(
    clippy::wildcard_imports,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    dead_code,
    reason = "split-out module preserves existing code while keeping files under the linecheck limit"
)]
use super::*;

#[test]
fn inline_prompt_overflow_some_when_composed_prompt_exceeds_inline_limit() {
    // A `{prompt}` agent (the shipped `claude` default) with a composed prompt over the inline
    // cap must be flagged, so the caller can skip a launch doomed to fail silently (#443).
    let mut routine = make_routine("Cmd Overflow Large Routine");
    routine.prompt = "x".repeat(MAX_INLINE_PROMPT_BYTES * 2);
    let agent = AgentCommand {
        command: "claude".to_string(),
        args: vec![
            "--permission-mode".to_string(),
            "auto".to_string(),
            "{prompt}".to_string(),
        ],
        instructions_file: "CLAUDE.md".to_string(),
        setup: None,
    };
    let overflow = inline_prompt_overflow(&routine, &agent);
    assert_eq!(overflow, Some(compose_prompt(&routine).len()));
    assert!(overflow.unwrap() > MAX_INLINE_PROMPT_BYTES);
}
