//! Tests for the `ui_*` tool handlers, run against the fake bridge.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::collections::BTreeMap;
use std::sync::Arc;

use gpui_mcp_protocol::{
    BridgeResult, InputCommand, NodeState, Operation, Point, PointerCommand, PointerScrollDelta,
    Rect, Role, TextInfo, UiNode, UiTree, ValueInfo, WireRequest,
};

use super::{
    ClickArgs, FindArgs, IdArgs, KeyArgs, ScrollArgs, SelectTargetArgs, ServerState, TextArgs,
    WaitArgs, click, element, element_bounds, find, focus, key, list_targets, ping, scroll,
    select_target, tree, type_text, wait_for_element,
};
use crate::bridge::fake::{
    Behavior, FakeBridge, OperationLog, failure, pong, recording, result, settled_frame,
};

/// One canned node with defaults for everything not given.
fn node(id: &str, parent: Option<&str>, role: Role, label: Option<&str>) -> UiNode {
    UiNode {
        id: id.to_owned(),
        parent: parent.map(str::to_owned),
        children: Vec::new(),
        role,
        label: label.map(str::to_owned),
        description: None,
        bounds: None,
        state: NodeState::default(),
        actions: Vec::new(),
        text: None,
        value: None,
        metadata: BTreeMap::new(),
    }
}

/// A small tree: a window with a button, a plain and a redacted text field,
/// a hidden node and enough fillers to exercise the find limit.
fn canned_tree() -> UiTree {
    let mut nodes = BTreeMap::new();
    let mut app = node("app", None, Role::Window, Some("ReqFlux"));
    app.bounds = Some(Rect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    });
    app.children = vec![
        "send-button".to_owned(),
        "url-field".to_owned(),
        "secret-field".to_owned(),
        "hidden-note".to_owned(),
    ];

    let mut send = node(
        "send-button",
        Some("app"),
        Role::Button,
        Some("Send request"),
    );
    send.bounds = Some(Rect {
        x: 10.0,
        y: 10.0,
        width: 80.0,
        height: 30.0,
    });

    let mut url = node("url-field", Some("app"), Role::TextInput, Some("URL"));
    url.bounds = Some(Rect {
        x: 100.0,
        y: 10.0,
        width: 300.0,
        height: 30.0,
    });
    url.text = Some(TextInfo {
        text: "https://reqflux.dev".to_owned(),
        caret: None,
        selection: None,
        redacted: false,
    });

    let mut secret = node("secret-field", Some("app"), Role::TextInput, Some("Token"));
    secret.bounds = Some(Rect {
        x: 100.0,
        y: 50.0,
        width: 300.0,
        height: 30.0,
    });
    secret.text = Some(TextInfo {
        text: String::new(),
        caret: None,
        selection: None,
        redacted: true,
    });
    secret.value = Some(ValueInfo {
        value: String::new(),
        min: None,
        max: None,
        step: None,
    });

    let mut hidden = node("hidden-note", Some("app"), Role::Text, Some("Hidden note"));
    hidden.state.visible = false;

    nodes.insert(app.id.clone(), app);
    nodes.insert(send.id.clone(), send);
    nodes.insert(url.id.clone(), url);
    nodes.insert(secret.id.clone(), secret);
    nodes.insert(hidden.id.clone(), hidden);
    for index in 0..210 {
        let filler = node(
            &format!("filler-{index:03}"),
            Some("app"),
            Role::Generic,
            None,
        );
        nodes.insert(filler.id.clone(), filler);
    }

    UiTree {
        generation: 7,
        roots: vec!["app".to_owned()],
        nodes,
        diagnostics: Vec::new(),
    }
}

/// A responder that answers `GetTree` with `tree` and everything else with an
/// ack, unless `refuse` rewrites the response for specific operations.
fn tree_bridge(log: OperationLog, tree: UiTree) -> FakeBridge {
    FakeBridge::new(
        "reqflux",
        0x01,
        recording(
            log,
            Arc::new(move |request| match &request.operation {
                Operation::GetTree => result(request, BridgeResult::Tree(tree.clone())),
                Operation::Ping => pong(request),
                Operation::GetPendingFrame => settled_frame(request),
                _ => result(request, BridgeResult::Ack),
            }),
        ),
    )
}

async fn tree_state(log: OperationLog) -> (FakeBridge, ServerState) {
    let bridge = tree_bridge(log, canned_tree());
    let state = ServerState::new(bridge.session("reqflux"));
    (bridge, state)
}

fn found(result: Result<String, super::ToolError>) -> serde_json::Value {
    let text = result.unwrap_or_else(|error| panic!("expected a result: {error}"));
    serde_json::from_str(&text).unwrap()
}

fn find_args(query: Option<&str>) -> FindArgs {
    FindArgs {
        query: query.map(str::to_owned),
        role: None,
        exact: None,
        visible_only: None,
        limit: None,
    }
}

// Connection tools.

#[tokio::test]
async fn ui_list_targets_reports_the_fake_with_selected() {
    let bridge = FakeBridge::new("reqflux", 0x01, Behavior::Respond(Arc::new(pong)));
    let mut state = ServerState::new(bridge.session("reqflux"));
    let value = found(list_targets(&mut state).await);
    let targets = value["targets"].as_array().unwrap();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0]["target_id"], bridge.started.target_id.as_str());
    assert_eq!(targets[0]["selected"], serde_json::json!(true));
}

#[tokio::test]
async fn ui_select_target_selects_the_target() {
    let bridge = FakeBridge::new("reqflux", 0x01, Behavior::Respond(Arc::new(pong)));
    let mut state = ServerState::new(bridge.session("reqflux"));
    let value = found(
        select_target(
            &mut state,
            SelectTargetArgs {
                target_id: bridge.started.target_id.as_str().to_owned(),
            },
        )
        .await,
    );
    assert_eq!(value["target_id"], bridge.started.target_id.as_str());
    assert_eq!(value["selected"], serde_json::json!(true));
}

#[tokio::test]
async fn ui_select_target_with_an_unknown_id_is_an_error() {
    let bridge = FakeBridge::new("reqflux", 0x01, Behavior::Respond(Arc::new(pong)));
    let mut state = ServerState::new(bridge.session("reqflux"));
    let error = select_target(
        &mut state,
        SelectTargetArgs {
            target_id: "reqflux-00000000000000ff".to_owned(),
        },
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("no live target"), "{error}");
}

#[tokio::test]
async fn ui_ping_returns_the_pong() {
    let bridge = FakeBridge::new("reqflux", 0x01, Behavior::Respond(Arc::new(pong)));
    let mut state = ServerState::new(bridge.session("reqflux"));
    let value = found(ping(&mut state).await);
    assert_eq!(value["ok"], serde_json::json!(true));
    assert_eq!(value["app_id"], serde_json::json!("reqflux"));
}

// Tree tools.

#[tokio::test]
async fn ui_tree_passes_the_bridge_tree_through() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log).await;
    let value = found(tree(&mut state).await);
    assert_eq!(value["generation"], serde_json::json!(7));
    assert_eq!(value["roots"][0], serde_json::json!("app"));
    assert!(value["nodes"]["send-button"].is_object());
}

#[tokio::test]
async fn ui_find_filters_by_query_substring() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log).await;
    let value = found(find(&mut state, find_args(Some("send"))).await);
    assert_eq!(value["count"], serde_json::json!(1));
    assert_eq!(value["elements"][0]["id"], serde_json::json!("send-button"));
    assert_eq!(value["elements"][0]["role"], serde_json::json!("button"));
}

#[tokio::test]
async fn ui_find_matches_case_insensitively_by_label_and_id() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log).await;
    let value = found(find(&mut state, find_args(Some("REQUEST"))).await);
    assert_eq!(value["elements"][0]["id"], serde_json::json!("send-button"));
    let value = found(find(&mut state, find_args(Some("SECRET"))).await);
    assert_eq!(
        value["elements"][0]["id"],
        serde_json::json!("secret-field")
    );
}

#[tokio::test]
async fn ui_find_filters_by_role() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log).await;
    let value = found(
        find(
            &mut state,
            FindArgs {
                role: Some("TEXT_INPUT".to_owned()),
                ..find_args(None)
            },
        )
        .await,
    );
    assert_eq!(value["count"], serde_json::json!(2));
    let value = found(
        find(
            &mut state,
            FindArgs {
                query: Some("url".to_owned()),
                role: Some("text_input".to_owned()),
                ..find_args(None)
            },
        )
        .await,
    );
    assert_eq!(value["count"], serde_json::json!(1));
    assert_eq!(value["elements"][0]["id"], serde_json::json!("url-field"));
}

#[tokio::test]
async fn ui_find_honours_visible_only() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log).await;
    let value = found(find(&mut state, find_args(Some("hidden"))).await);
    assert_eq!(
        value["count"],
        serde_json::json!(0),
        "hidden nodes are skipped by default"
    );
    let value = found(
        find(
            &mut state,
            FindArgs {
                visible_only: Some(false),
                ..find_args(Some("hidden"))
            },
        )
        .await,
    );
    assert_eq!(value["count"], serde_json::json!(1));
}

#[tokio::test]
async fn ui_find_limits_and_caps_at_200() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log).await;
    let value = found(find(&mut state, find_args(None)).await);
    assert_eq!(
        value["count"],
        serde_json::json!(50),
        "the default limit is 50"
    );
    let value = found(
        find(
            &mut state,
            FindArgs {
                limit: Some(2),
                ..find_args(None)
            },
        )
        .await,
    );
    assert_eq!(value["count"], serde_json::json!(2));
    let value = found(
        find(
            &mut state,
            FindArgs {
                limit: Some(500),
                ..find_args(None)
            },
        )
        .await,
    );
    assert_eq!(
        value["count"],
        serde_json::json!(200),
        "the limit is capped at 200"
    );
}

#[tokio::test]
async fn ui_find_exact_requires_the_whole_label() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log).await;
    let value = found(
        find(
            &mut state,
            FindArgs {
                query: Some("Send request".to_owned()),
                exact: Some(true),
                ..find_args(None)
            },
        )
        .await,
    );
    assert_eq!(value["count"], serde_json::json!(1));
    let value = found(
        find(
            &mut state,
            FindArgs {
                query: Some("send".to_owned()),
                exact: Some(true),
                ..find_args(None)
            },
        )
        .await,
    );
    assert_eq!(value["count"], serde_json::json!(0));
}

#[tokio::test]
async fn ui_element_returns_the_node() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log).await;
    let value = found(
        element(
            &mut state,
            IdArgs {
                id: "url-field".to_owned(),
            },
        )
        .await,
    );
    assert_eq!(value["parent"], serde_json::json!("app"));
    assert_eq!(value["role"], serde_json::json!("text_input"));
    assert_eq!(
        value["text"]["text"],
        serde_json::json!("https://reqflux.dev")
    );
    let value = found(
        element(
            &mut state,
            IdArgs {
                id: "app".to_owned(),
            },
        )
        .await,
    );
    assert_eq!(value["children"][0], serde_json::json!("send-button"));
}

#[tokio::test]
async fn ui_element_keeps_redacted_text_withheld() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log).await;
    let value = found(
        element(
            &mut state,
            IdArgs {
                id: "secret-field".to_owned(),
            },
        )
        .await,
    );
    assert_eq!(
        value["text"]["text"],
        serde_json::json!(""),
        "redacted text stays empty"
    );
    assert_eq!(value["text"]["redacted"], serde_json::json!(true));
    assert_eq!(value["value"]["value"], serde_json::json!(""));
}

#[tokio::test]
async fn ui_element_with_an_unknown_id_is_an_error() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log).await;
    let error = element(
        &mut state,
        IdArgs {
            id: "absent".to_owned(),
        },
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("absent"), "{error}");
}

#[tokio::test]
async fn ui_element_bounds_returns_bounds() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log).await;
    let value = found(
        element_bounds(
            &mut state,
            IdArgs {
                id: "send-button".to_owned(),
            },
        )
        .await,
    );
    assert_eq!(value["bounds"]["x"], serde_json::json!(10.0));
    assert_eq!(value["bounds"]["width"], serde_json::json!(80.0));
}

#[tokio::test]
async fn ui_wait_for_element_returns_a_match() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log).await;
    let value = found(
        wait_for_element(
            &mut state,
            WaitArgs {
                query: Some("send".to_owned()),
                role: None,
                exact: None,
                timeout_ms: Some(1_000),
            },
        )
        .await,
    );
    assert_eq!(value["element"]["id"], serde_json::json!("send-button"));
    assert!(value["elapsed_ms"].is_u64());
}

#[tokio::test]
async fn ui_wait_for_element_times_out_with_a_clear_message() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log).await;
    let error = wait_for_element(
        &mut state,
        WaitArgs {
            query: Some("absent".to_owned()),
            role: None,
            exact: None,
            timeout_ms: Some(0),
        },
    )
    .await
    .unwrap_err();
    let message = error.to_string();
    assert!(message.contains("no element matching"), "{message}");
    assert!(message.contains("0 ms"), "{message}");
}

// Input tools.

#[tokio::test]
async fn ui_click_sends_pointer_input_at_the_center() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log.clone()).await;
    let value = found(
        click(
            &mut state,
            ClickArgs {
                id: "send-button".to_owned(),
                button: None,
                count: Some(2),
            },
        )
        .await,
    );
    assert_eq!(value["ok"], serde_json::json!(true));
    assert_eq!(value["action"], serde_json::json!("clicked"));

    let center = Point { x: 50.0, y: 25.0 };
    let ops = log.lock().unwrap();
    assert_eq!(
        *ops,
        vec![
            Operation::GetTree,
            Operation::PointerInput {
                command: PointerCommand::MouseDown {
                    point: center,
                    button: gpui_mcp_protocol::MouseButton::Left,
                    click_count: 1,
                },
            },
            Operation::PointerInput {
                command: PointerCommand::MouseUp {
                    point: center,
                    button: gpui_mcp_protocol::MouseButton::Left,
                    click_count: 1,
                },
            },
            Operation::PointerInput {
                command: PointerCommand::MouseDown {
                    point: center,
                    button: gpui_mcp_protocol::MouseButton::Left,
                    click_count: 2,
                },
            },
            Operation::PointerInput {
                command: PointerCommand::MouseUp {
                    point: center,
                    button: gpui_mcp_protocol::MouseButton::Left,
                    click_count: 2,
                },
            },
            Operation::GetPendingFrame,
        ],
        "a double click is one press and release per click count",
    );
}

#[tokio::test]
async fn ui_click_rejects_a_bad_count_or_button() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log).await;
    let error = click(
        &mut state,
        ClickArgs {
            id: "send-button".to_owned(),
            button: None,
            count: Some(0),
        },
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("count"), "{error}");
    let error = click(
        &mut state,
        ClickArgs {
            id: "send-button".to_owned(),
            button: Some("wheel".to_owned()),
            count: None,
        },
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("button"), "{error}");
}

#[tokio::test]
async fn ui_click_without_bounds_is_an_error() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log).await;
    let error = click(
        &mut state,
        ClickArgs {
            id: "hidden-note".to_owned(),
            button: None,
            count: None,
        },
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("bounds"), "{error}");
}

#[tokio::test]
async fn ui_type_reaches_the_bridge() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log.clone()).await;
    let value = found(
        type_text(
            &mut state,
            TextArgs {
                text: "hello".to_owned(),
            },
        )
        .await,
    );
    assert_eq!(value["action"], serde_json::json!("typed"));
    assert_eq!(
        *log.lock().unwrap(),
        vec![
            Operation::Input {
                command: InputCommand::TypeText {
                    text: "hello".to_owned()
                },
            },
            Operation::GetPendingFrame,
        ]
    );
}

#[tokio::test]
async fn ui_type_into_a_redacted_element_surfaces_the_bridge_refusal() {
    let log = OperationLog::default();
    let bridge = FakeBridge::new(
        "reqflux",
        0x01,
        recording(
            log,
            Arc::new(|request| match &request.operation {
                Operation::Input {
                    command: InputCommand::TypeText { .. },
                } => failure(request, "cannot type into a redacted element"),
                Operation::GetPendingFrame => settled_frame(request),
                _ => result(request, BridgeResult::Ack),
            }),
        ),
    );
    let mut state = ServerState::new(bridge.session("reqflux"));
    let error = type_text(
        &mut state,
        TextArgs {
            text: "leak".to_owned(),
        },
    )
    .await
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("cannot type into a redacted element"),
        "{error}"
    );
}

#[tokio::test]
async fn ui_key_reaches_the_bridge() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log.clone()).await;
    found(
        key(
            &mut state,
            KeyArgs {
                keystroke: "ctrl-a".to_owned(),
            },
        )
        .await,
    );
    assert_eq!(
        *log.lock().unwrap(),
        vec![
            Operation::Input {
                command: InputCommand::Key {
                    keystroke: "ctrl-a".to_owned()
                },
            },
            Operation::GetPendingFrame,
        ]
    );
}

#[tokio::test]
async fn ui_focus_reaches_the_bridge() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log.clone()).await;
    let value = found(
        focus(
            &mut state,
            IdArgs {
                id: "url-field".to_owned(),
            },
        )
        .await,
    );
    assert_eq!(value["action"], serde_json::json!("focused"));
    assert_eq!(
        *log.lock().unwrap(),
        vec![
            Operation::Focus {
                node_id: "url-field".to_owned(),
            },
            Operation::GetPendingFrame,
        ]
    );
}

#[tokio::test]
async fn ui_scroll_reaches_the_bridge() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log.clone()).await;
    found(
        scroll(
            &mut state,
            ScrollArgs {
                id: Some("url-field".to_owned()),
                x: None,
                y: None,
                delta_x: None,
                delta_y: Some(-120.0),
            },
        )
        .await,
    );
    assert_eq!(
        *log.lock().unwrap(),
        vec![
            Operation::GetTree,
            Operation::PointerInput {
                command: PointerCommand::ScrollWheel {
                    point: Point { x: 250.0, y: 25.0 },
                    delta: PointerScrollDelta::Pixels {
                        delta_x: 0.0,
                        delta_y: -120.0
                    },
                },
            },
            Operation::GetPendingFrame,
        ]
    );
}

#[tokio::test]
async fn ui_scroll_without_a_point_is_an_error() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log).await;
    let error = scroll(
        &mut state,
        ScrollArgs {
            id: None,
            x: None,
            y: None,
            delta_x: None,
            delta_y: None,
        },
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("id or both x and y"), "{error}");
}

// Error paths.

#[tokio::test]
async fn a_bridge_error_comes_back_with_its_message() {
    let bridge = FakeBridge::new(
        "reqflux",
        0x01,
        Behavior::Respond(Arc::new(|request| {
            failure(request, "bridge is shutting down")
        })),
    );
    let mut state = ServerState::new(bridge.session("reqflux"));
    let error = ping(&mut state).await.unwrap_err();
    assert!(
        error.to_string().contains("bridge is shutting down"),
        "{error}"
    );
}

#[tokio::test]
async fn a_missing_target_comes_back_with_the_app_id() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut state = ServerState::new(crate::bridge::BridgeSession::new(
        dir.path().to_path_buf(),
        gpui_mcp_protocol::AppId::new("reqflux").unwrap(),
    ));
    let error = ping(&mut state).await.unwrap_err();
    assert!(
        error.to_string().contains("no reqflux app is running"),
        "{error}"
    );
}

#[tokio::test]
async fn ui_list_targets_with_no_target_returns_an_empty_list() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut state = ServerState::new(crate::bridge::BridgeSession::new(
        dir.path().to_path_buf(),
        gpui_mcp_protocol::AppId::new("reqflux").unwrap(),
    ));
    let value = found(list_targets(&mut state).await);
    assert_eq!(value["targets"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn json_results_carry_the_documented_fields() {
    let log = OperationLog::default();
    let (_bridge, mut state) = tree_state(log).await;

    let value = found(list_targets(&mut state).await);
    for field in [
        "target_id",
        "app_id",
        "app_name",
        "pid",
        "capabilities",
        "selected",
    ] {
        assert!(
            value["targets"][0].get(field).is_some(),
            "target summary has {field}"
        );
    }
    let value = found(tree(&mut state).await);
    for field in ["generation", "roots", "nodes"] {
        assert!(value.get(field).is_some(), "tree has {field}");
    }
    let value = found(find(&mut state, find_args(None)).await);
    for field in ["id", "label", "role", "bounds", "state"] {
        assert!(
            value["elements"][0].get(field).is_some(),
            "find element has {field}"
        );
    }
    let value = found(
        element(
            &mut state,
            IdArgs {
                id: "url-field".to_owned(),
            },
        )
        .await,
    );
    for field in [
        "id", "parent", "children", "role", "label", "bounds", "state", "text", "value",
    ] {
        assert!(value.get(field).is_some(), "element has {field}");
    }
    let value = found(
        element_bounds(
            &mut state,
            IdArgs {
                id: "url-field".to_owned(),
            },
        )
        .await,
    );
    for field in ["id", "bounds"] {
        assert!(value.get(field).is_some(), "bounds result has {field}");
    }
}

#[tokio::test]
async fn an_input_that_settles_badly_is_still_delivered() {
    let log = OperationLog::default();
    let bridge = FakeBridge::new(
        "reqflux",
        0x01,
        recording(
            log.clone(),
            Arc::new(|request: &WireRequest| match &request.operation {
                Operation::GetTree => result(request, BridgeResult::Tree(canned_tree())),
                Operation::GetPendingFrame => failure(request, "the bridge is shutting down"),
                _ => result(request, BridgeResult::Ack),
            }),
        ),
    );
    let mut state = ServerState::new(bridge.session("reqflux"));
    let value = found(
        type_text(
            &mut state,
            TextArgs {
                text: "hello".to_owned(),
            },
        )
        .await,
    );
    assert_eq!(value["ok"], serde_json::json!(true));
    assert_eq!(value["settled"], serde_json::json!(false));
    let ops = log.lock().unwrap().clone();
    assert!(
        ops.iter().any(|operation| matches!(
            operation,
            Operation::Input {
                command: InputCommand::TypeText { .. }
            }
        )),
        "the text reached the bridge: {ops:?}"
    );
}

#[tokio::test]
async fn a_node_marked_redacted_never_hands_its_text_out() {
    // The bridge withholds redacted text itself; this proves the tool does not depend on that
    // and blanks the value even when it is present.
    let log = OperationLog::default();
    let mut tree = canned_tree();
    if let Some(node) = tree.nodes.get_mut("secret-field") {
        node.text = Some(TextInfo {
            text: "SECRET-VALUE".to_owned(),
            caret: None,
            selection: None,
            redacted: true,
        });
    }
    let bridge = tree_bridge(log, tree);
    let mut state = ServerState::new(bridge.session("reqflux"));
    let value = found(
        element(
            &mut state,
            IdArgs {
                id: "secret-field".to_owned(),
            },
        )
        .await,
    );
    assert_eq!(value["text"]["text"], serde_json::json!(""));
    assert_eq!(value["text"]["redacted"], serde_json::json!(true));
}
