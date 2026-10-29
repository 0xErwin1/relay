//! The response view: the tab row (S5), the body and headers (S6), the error banner and
//! Retry (S7) and the empty state, against a local HTTP server.

use gpui::{Pixels, TestAppContext, VisualTestContext};

use super::send_tests::{PING, Server, click, closed_port, render, response, serve, settle};

fn present(cx: &mut VisualTestContext, selector: &str) -> gpui::Bounds<Pixels> {
    match cx.debug_bounds(Box::leak(selector.to_owned().into_boxed_str())) {
        Some(bounds) => bounds,
        None => panic!("{selector} is missing"),
    }
}

fn absent(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    cx.debug_bounds(selector).is_none()
}

/// The tops of the body lines `1..=count`, which must all be present.
fn line_tops(cx: &mut VisualTestContext, count: usize) -> Vec<Pixels> {
    (1..=count)
        .map(|n| present(cx, &format!("response-body-line-{n}")).origin.y)
        .collect()
}

#[gpui::test]
fn a_request_never_sent_shows_the_empty_state(cx: &mut TestAppContext) {
    let (_, cx) = render(cx, serve(Server::Ok));
    let request = present(cx, "request-pane");
    let empty = present(cx, "response-empty");
    assert!(empty.origin.y >= request.bottom());
    assert!(absent(cx, "response-status"));
    assert!(absent(cx, "response-body"));
}

#[gpui::test]
fn a_json_response_shows_its_status_meta_and_pretty_body(cx: &mut TestAppContext) {
    let (shell, cx) = render(cx, serve(Server::Ok));
    click(cx, "request-send");
    assert_eq!(settle(cx, &shell, PING).as_deref(), Some("200 OK"));

    present(cx, "response-status");
    present(cx, "response-meta");
    present(cx, "response-tab-body");
    present(cx, "response-view-pretty");
    assert!(absent(cx, "response-empty"));
    let body = present(cx, "response-body");
    let tops = line_tops(cx, 6);
    assert!(tops.windows(2).all(|pair| pair[0] < pair[1]), "{tops:?}");
    assert!(tops.iter().all(|top| *top >= body.origin.y));
    assert!(absent(cx, "response-body-line-7"));

    click(cx, "response-view-raw");
    line_tops(cx, 1);
    assert!(absent(cx, "response-body-line-2"));
}

#[gpui::test]
fn the_headers_tab_lists_the_response_headers(cx: &mut TestAppContext) {
    let (shell, cx) = render(cx, serve(Server::Ok));
    click(cx, "request-send");
    settle(cx, &shell, PING);

    click(cx, "response-tab-headers");
    for n in 1..=3 {
        present(cx, &format!("response-headers-row-{n}"));
    }
    assert!(absent(cx, "response-headers-row-4"));
    assert!(absent(cx, "response-body"));
}

#[gpui::test]
fn a_connection_error_shows_the_banner(cx: &mut TestAppContext) {
    let (shell, cx) = render(cx, closed_port());
    click(cx, "request-send");
    settle(cx, &shell, PING);

    present(cx, "response-status");
    present(cx, "response-meta");
    present(cx, "response-error");
    present(cx, "response-retry");
    assert!(absent(cx, "response-body"));
}

#[gpui::test]
fn retry_sends_again(cx: &mut TestAppContext) {
    let (shell, cx) = render(cx, serve(Server::Silent));
    click(cx, "request-send");
    present(cx, "response-progress");
    present(cx, "response-elapsed");
    click(cx, "request-cancel");
    assert_eq!(
        settle(cx, &shell, PING).as_deref(),
        Some("Failed: request cancelled")
    );
    assert!(absent(cx, "response-progress"));

    click(cx, "response-retry");
    assert_eq!(response(cx, &shell, PING).as_deref(), Some("Sending"));
    // The failed send stays visible, dimmed, under the progress bar.
    present(cx, "response-progress");
    present(cx, "response-error");
}
