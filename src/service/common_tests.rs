use super::*;

#[test]
fn current_exe_error_formats_the_path_failure() {
    let err = current_exe_path(|| Err(std::io::Error::other("boom"))).unwrap_err();
    assert!(err
        .to_string()
        .contains("failed to resolve current executable path: boom"));
}
