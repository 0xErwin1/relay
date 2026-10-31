use super::{RequestEntry, score};

fn entry(method: &str, name: &str, url: &str) -> RequestEntry {
    RequestEntry {
        id: name.to_owned(),
        method: method.to_owned(),
        name: name.to_owned(),
        url: url.to_owned(),
        path: "API".to_owned(),
    }
}

#[test]
fn a_subsequence_matches_and_a_missing_character_does_not() {
    assert!(score("hlth", "Health").is_some());
    assert_eq!(score("hx", "Health"), None);
    assert_eq!(score("healthy", "Health"), None, "the query is longer");
}

#[test]
fn matching_ignores_case() {
    assert_eq!(score("HEALTH", "health"), score("health", "Health"));
    assert!(score("HeAlTh", "health").is_some());
}

#[test]
fn an_empty_query_matches_everything() {
    assert_eq!(score("", "Health"), Some(0));
    assert_eq!(score("  ", "Health"), Some(0));
}

#[test]
fn contiguous_runs_score_higher_than_scattered_characters() {
    let contiguous = score("user", "Users");
    let scattered = score("user", "Uxsxexr");
    assert!(contiguous > scattered, "{contiguous:?} <= {scattered:?}");
}

#[test]
fn word_starts_score_higher_than_inner_characters() {
    let start = score("us", "List users");
    let inner = score("us", "Status");
    assert!(start > inner, "{start:?} <= {inner:?}");
}

#[test]
fn get_users_finds_the_get_users_request_first() {
    let wanted = entry("GET", "Users", "https://api.test/users");
    let other = entry("POST", "Upload", "https://api.test/files");
    let unrelated = entry("GET", "Health", "https://api.test/health");
    assert!(wanted.score("get users").is_some());
    assert_eq!(other.score("get users"), None);
    assert!(wanted.score("get users") > unrelated.score("get users"));
}

#[test]
fn a_request_matches_by_url_as_written() {
    let request = entry("GET", "List", "{{base}}/v2/accounts");
    assert!(request.score("accounts").is_some());
    assert!(request.score("get v2 acc").is_some(), "method and URL");
    assert_eq!(request.score("orders"), None);
}

#[test]
fn a_request_matches_by_method() {
    assert!(entry("DELETE", "Drop", "/x").score("delete").is_some());
    assert_eq!(entry("GET", "Drop", "/x").score("put"), None);
}
