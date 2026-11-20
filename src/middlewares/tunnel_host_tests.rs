use super::is_tunnel_host;

fn suffixes() -> Vec<String> {
    vec!["trycloudflare.com".to_string()]
}

#[test]
fn subdomain_of_suffix_matches() {
    assert!(is_tunnel_host("abc-def.trycloudflare.com", &suffixes()));
}

#[test]
fn deep_subdomain_of_suffix_matches() {
    assert!(is_tunnel_host("a.b.trycloudflare.com", &suffixes()));
}

#[test]
fn match_ignores_port_and_case() {
    assert!(is_tunnel_host("ABC.TryCloudflare.com:443", &suffixes()));
}

#[test]
fn leading_dot_in_configured_suffix_is_accepted() {
    let dotted = vec![".trycloudflare.com".to_string()];
    assert!(is_tunnel_host("abc.trycloudflare.com", &dotted));
}

#[test]
fn bare_suffix_does_not_match() {
    assert!(!is_tunnel_host("trycloudflare.com", &suffixes()));
    assert!(!is_tunnel_host(".trycloudflare.com", &suffixes()));
}

#[test]
fn lookalike_suffix_does_not_match() {
    assert!(!is_tunnel_host("eviltrycloudflare.com", &suffixes()));
    assert!(!is_tunnel_host(
        "abc.trycloudflare.com.attacker.example",
        &suffixes()
    ));
}

#[test]
fn empty_configured_suffix_matches_nothing() {
    let empty = vec![String::new(), "  .  ".to_string()];
    assert!(!is_tunnel_host("abc.trycloudflare.com", &empty));
}

#[test]
fn no_configured_suffixes_matches_nothing() {
    assert!(!is_tunnel_host("abc.trycloudflare.com", &[]));
}

#[test]
fn trailing_dot_host_does_not_match() {
    assert!(!is_tunnel_host("abc.trycloudflare.com.", &suffixes()));
    assert!(!is_tunnel_host("abc.trycloudflare.com.:443", &suffixes()));
    assert!(!is_tunnel_host("trycloudflare.com.", &suffixes()));
}
