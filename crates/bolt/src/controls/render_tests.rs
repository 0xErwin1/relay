//! Renders each control and checks its bounds and what UI automation sees.

use gpui::{
    Context, IntoElement, Render, TestAppContext, VisualTestContext, Window, div, prelude::*, px,
    size,
};

use super::*;
use crate::icon::IconName;
use crate::test_support::A11yRecorder;

fn bounds_of(cx: &mut VisualTestContext, selector: &'static str) -> gpui::Bounds<gpui::Pixels> {
    match cx.debug_bounds(selector) {
        Some(bounds) => bounds,
        None => panic!("{selector} was not laid out"),
    }
}

struct Buttons;

impl Render for Buttons {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_start()
            .child(IconButton::new("save", IconName::Save, "Save"))
            .child(IconButton::new("add", IconName::Plus, "New request").raised())
            .child(Keycap::new("hint", "Ctrl Shift P"))
    }
}

#[gpui::test]
fn icon_buttons_are_32_by_30_and_labeled(cx: &mut TestAppContext) {
    let (_view, cx) = cx.add_window_view(|_, _| Buttons);
    cx.run_until_parked();

    assert_eq!(bounds_of(cx, "save").size, size(px(32.0), px(30.0)));
    assert_eq!(bounds_of(cx, "add").size, size(px(32.0), px(30.0)));
    let a11y = A11yRecorder::observe(cx);
    assert_eq!(a11y.label_of("save").as_deref(), Some("Save"));
    assert_eq!(a11y.label_of("add").as_deref(), Some("New request"));
}

#[gpui::test]
fn keycaps_fit_their_key_and_are_labeled(cx: &mut TestAppContext) {
    let (_view, cx) = cx.add_window_view(|_, _| Buttons);
    cx.run_until_parked();

    let keycap = bounds_of(cx, "hint");
    assert_eq!(keycap.size.height, px(18.0));
    assert!(keycap.size.width > px(12.0));
    let a11y = A11yRecorder::observe(cx);
    assert_eq!(a11y.label_of("hint").as_deref(), Some("Ctrl Shift P"));
}

struct Shell;

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                Field::new("search", "Search or run a command")
                    .leading_icon(IconName::Search)
                    .trailing_keycap("Ctrl Shift P")
                    .w(px(420.0)),
            )
            .child(Field::new("plain", "Filter requests").w(px(200.0)))
    }
}

#[gpui::test]
fn fields_are_30_high_at_the_width_they_were_given(cx: &mut TestAppContext) {
    let (_view, cx) = cx.add_window_view(|_, _| Shell);
    cx.run_until_parked();

    assert_eq!(bounds_of(cx, "search").size, size(px(420.0), px(30.0)));
    assert_eq!(bounds_of(cx, "plain").size, size(px(200.0), px(30.0)));
    let search = bounds_of(cx, "search");
    let keycap = bounds_of(cx, "search-keycap");
    assert!(
        search.contains(&keycap.origin),
        "{keycap:?} outside {search:?}"
    );
    assert_eq!(keycap.right(), search.right() - px(10.0));
    assert!(cx.debug_bounds("plain-keycap").is_none());

    let a11y = A11yRecorder::observe(cx);
    assert_eq!(
        a11y.label_of("search").as_deref(),
        Some("Search or run a command")
    );
    assert_eq!(
        a11y.label_of("search-keycap").as_deref(),
        Some("Ctrl Shift P")
    );
    assert_eq!(a11y.label_of("plain").as_deref(), Some("Filter requests"));
}
