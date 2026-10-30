//! The redacted copy of a prepared request that history records (history S3).

#![allow(clippy::panic, reason = "test helpers fail the calling test")]

mod common;

use common::{collection, options, post, prepare, prepare_with, request, single, workspace};
use pretty_assertions::assert_eq;
use reqflux_core::model::{Auth, Body, KeyValue, RawLanguage, Variable};
use reqflux_engine::{MASK, SentBody, SentRequest};

fn secret(key: &str, value: &str) -> Variable {
    Variable {
        secret: true,
        ..Variable::new(key, value)
    }
}

fn header<'a>(sent: &'a SentRequest, name: &str) -> &'a str {
    match sent.headers.iter().find(|(key, _)| key == name) {
        Some((_, value)) => value,
        None => panic!("no header {name} in {:?}", sent.headers),
    }
}

fn raw(text: &str) -> Body {
    Body::Raw {
        language: RawLanguage::Text,
        text: text.to_owned(),
    }
}

#[test]
fn the_mask_is_eight_bullets() {
    assert_eq!(MASK, "••••••••");
}

#[test]
fn a_secret_variable_is_masked_in_the_header_query_and_body() {
    let mut doc = post("https://api.test/v1?static={{plain}}");
    doc.headers = vec![KeyValue::new("X-Key", "key {{token}}")];
    doc.params = vec![KeyValue::new("t", "{{token}}")];
    doc.body = raw("token={{token}}; plain={{plain}}");
    let ws = workspace(
        vec![secret("token", "s3cr3t"), Variable::new("plain", "visible")],
        vec![collection("c", vec![request("r", doc)])],
    );

    let sent = prepare(&ws).sent();

    assert_eq!(header(&sent, "X-Key"), format!("key {MASK}"));
    assert_eq!(
        sent.url,
        format!("https://api.test/v1?static=visible&t={MASK}")
    );
    assert_eq!(
        sent.body,
        SentBody::Text(format!("token={MASK}; plain=visible"))
    );
}

#[test]
fn a_query_secret_with_reserved_characters_is_masked_once_encoded() {
    let mut doc = post("https://api.test/");
    doc.params = vec![KeyValue::new("t", "{{token}}")];
    let ws = workspace(
        vec![secret("token", "a b&c")],
        vec![collection("c", vec![request("r", doc)])],
    );

    let sent = prepare(&ws).sent();

    assert!(!sent.url.contains("a%20b%26c"), "{}", sent.url);
    assert!(!sent.url.contains("a b&c"), "{}", sent.url);
}

#[test]
fn a_literal_bearer_token_is_masked() {
    let mut doc = post("https://api.test/");
    doc.auth = Auth::Bearer {
        token: "literal-token".to_owned(),
    };
    let sent = prepare(&single(doc)).sent();

    assert_eq!(header(&sent, "Authorization"), format!("Bearer {MASK}"));
}

#[test]
fn a_literal_basic_password_is_masked() {
    let mut doc = post("https://api.test/");
    doc.auth = Auth::Basic {
        username: "user".to_owned(),
        password: "pass".to_owned(),
    };
    let sent = prepare(&single(doc)).sent();

    assert_eq!(header(&sent, "Authorization"), format!("Basic {MASK}"));
}

#[test]
fn a_literal_api_key_value_is_masked_but_not_its_name() {
    let mut doc = post("https://api.test/");
    doc.auth = Auth::ApiKey {
        key: "X-Api-Key".to_owned(),
        value: "k-123".to_owned(),
        location: reqflux_core::model::ApiKeyLocation::Header,
    };
    let sent = prepare(&single(doc)).sent();

    assert_eq!(header(&sent, "X-Api-Key"), MASK);
}

#[test]
fn a_non_secret_value_is_not_masked() {
    let mut doc = post("https://api.test/{{plain}}");
    doc.headers = vec![KeyValue::new("X-Plain", "{{plain}}")];
    let ws = workspace(
        vec![Variable::new("plain", "visible")],
        vec![collection("c", vec![request("r", doc)])],
    );

    let sent = prepare(&ws).sent();

    assert_eq!(sent.url, "https://api.test/visible");
    assert_eq!(header(&sent, "X-Plain"), "visible");
}

#[test]
fn a_secret_inside_another_word_is_still_masked() {
    let mut doc = post("https://api.test/");
    doc.body = raw("prefix{{token}}suffix and abcxyz");
    let ws = workspace(
        vec![secret("token", "abc")],
        vec![collection("c", vec![request("r", doc)])],
    );

    let sent = prepare(&ws).sent();

    // Text replacement masks every occurrence, also one the secret did not produce.
    assert_eq!(
        sent.body,
        SentBody::Text(format!("prefix{MASK}suffix and {MASK}xyz"))
    );
}

#[test]
fn a_local_override_of_a_secret_is_masked_with_its_resolved_value() {
    let mut doc = post("https://api.test/");
    doc.headers = vec![KeyValue::new("X-Key", "{{token}}")];
    let ws = workspace(
        vec![secret("token", "stored")],
        vec![collection("c", vec![request("r", doc)])],
    );

    let sent = prepare_with(&ws, "c/r", &options(None, &[("token", "override")])).sent();

    assert_eq!(header(&sent, "X-Key"), MASK);
}

#[test]
fn form_bodies_are_text_and_multipart_and_files_are_markers() {
    let mut doc = post("https://api.test/");
    doc.body = Body::Urlencoded {
        fields: vec![KeyValue::new("a", "1 2"), KeyValue::new("b", "{{token}}")],
    };
    let ws = workspace(
        vec![secret("token", "s3cr3t")],
        vec![collection("c", vec![request("r", doc.clone())])],
    );
    assert_eq!(
        prepare(&ws).sent().body,
        SentBody::Text(format!("a=1+2&b={MASK}"))
    );

    doc.body = Body::Binary {
        path: "data.bin".to_owned(),
    };
    let sent = prepare(&single(doc)).sent();
    assert!(matches!(sent.body, SentBody::Omitted(_)), "{:?}", sent.body);
}
