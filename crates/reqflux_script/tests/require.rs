//! `require()` serves the libraries Postman's sandbox ships (S15).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests fail the run"
)]

use std::collections::BTreeMap;

use reqflux_script::{RequestView, ScriptContext, ScriptStage, run};

fn context() -> ScriptContext {
    ScriptContext {
        stage: ScriptStage::PreRequest,
        request: RequestView {
            method: "GET".into(),
            url: "https://api.test/".into(),
            headers: BTreeMap::new(),
            body: None,
        },
        variables: BTreeMap::new(),
        response: None,
        send_request: None,
    }
}

fn logged(script: &str) -> String {
    let outcome = run(script, &context()).unwrap_or_else(|error| panic!("{error}"));
    outcome
        .console
        .first()
        .map(|entry| entry.message.clone())
        .unwrap_or_default()
}

#[test]
fn lodash_is_available() {
    assert_eq!(
        logged(r#"const _ = require("lodash"); console.log(_.chunk([1,2,3,4], 2).length);"#),
        "2"
    );
}

#[test]
fn moment_is_available() {
    assert_eq!(
        logged(
            r#"const moment = require("moment"); console.log(moment("2020-05-06").format("YYYY-MM"));"#
        ),
        "2020-05"
    );
}

#[test]
fn chai_expect_is_available() {
    assert_eq!(
        logged(
            r#"const { expect } = require("chai");
            try { expect(1).to.equal(2); console.log("no"); } catch (error) { console.log("caught"); }"#
        ),
        "caught"
    );
}

#[test]
fn a_module_is_compiled_once() {
    assert_eq!(
        logged(
            r#"const first = require("lodash"); const second = require("lodash");
            console.log(first === second);"#
        ),
        "true"
    );
}

#[test]
fn an_unknown_module_fails_with_its_name() {
    let error = run(r#"require("jquery");"#, &context()).unwrap_err();
    assert!(error.to_string().contains("jquery"), "{error}");
}
