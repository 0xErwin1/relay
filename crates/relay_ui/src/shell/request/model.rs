//! The request view without GPUI: the sub-tabs, their row counts and the unresolved notice.

use relay_core::model::KeyValue;
use relay_engine::Inspection;

/// A sub-tab of the request view, in bar order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SubTab {
    Params,
    Headers,
    Body,
    Auth,
    Scripts,
    Settings,
}

impl SubTab {
    /// Every sub-tab, left to right.
    pub(crate) const ALL: [Self; 6] = [
        Self::Params,
        Self::Headers,
        Self::Body,
        Self::Auth,
        Self::Scripts,
        Self::Settings,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Params => "Params",
            Self::Headers => "Headers",
            Self::Body => "Body",
            Self::Auth => "Auth",
            Self::Scripts => "Scripts",
            Self::Settings => "Settings",
        }
    }

    /// The automation id, `request-tab-<lowercase label>`.
    pub(crate) fn element_id(self) -> String {
        format!("request-tab-{}", self.label().to_lowercase())
    }
}

/// An open request: what the engine inspected and the selected sub-tab.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RequestState {
    pub(crate) inspection: Inspection,
    pub(crate) tab: SubTab,
}

impl RequestState {
    /// The state of a newly opened request, on the Headers sub-tab.
    pub(crate) fn new(inspection: Inspection) -> Self {
        Self {
            inspection,
            tab: SubTab::Headers,
        }
    }

    /// The number of enabled rows `tab` shows next to its label: Params and Headers only.
    pub(crate) fn count(&self, tab: SubTab) -> Option<usize> {
        let request = &self.inspection.request.request;
        match tab {
            SubTab::Params => Some(enabled_count(&request.params)),
            SubTab::Headers => Some(enabled_count(&request.headers)),
            SubTab::Body | SubTab::Auth | SubTab::Scripts | SubTab::Settings => None,
        }
    }
}

pub(crate) fn enabled_count(rows: &[KeyValue]) -> usize {
    rows.iter().filter(|row| row.enabled).count()
}

/// `<n> unresolved variable(s)`, or `None` when nothing is unresolved.
pub(crate) fn unresolved_text(count: usize) -> Option<String> {
    match count {
        0 => None,
        1 => Some("1 unresolved variable".to_owned()),
        _ => Some(format!("{count} unresolved variables")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sub_tabs_are_in_bar_order_with_lowercase_ids() {
        let ids: Vec<_> = SubTab::ALL.iter().map(|tab| tab.element_id()).collect();
        assert_eq!(
            ids,
            [
                "request-tab-params",
                "request-tab-headers",
                "request-tab-body",
                "request-tab-auth",
                "request-tab-scripts",
                "request-tab-settings",
            ]
        );
    }

    #[test]
    fn counts_only_enabled_rows() {
        let off = KeyValue {
            enabled: false,
            ..KeyValue::new("b", "2")
        };
        assert_eq!(enabled_count(&[]), 0);
        assert_eq!(
            enabled_count(&[KeyValue::new("a", "1"), off, KeyValue::new("c", "3")]),
            2
        );
    }

    #[test]
    fn unresolved_text_agrees_with_the_count() {
        assert_eq!(unresolved_text(0), None);
        assert_eq!(unresolved_text(1).as_deref(), Some("1 unresolved variable"));
        assert_eq!(
            unresolved_text(2).as_deref(),
            Some("2 unresolved variables")
        );
    }
}
