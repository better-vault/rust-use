use super::{Selection, SelectionMode, action, dispatch, param};
use crate::{ActionError, ActionSpec, Args, Behavior, Describe, Value};

/// An ordered set of tabs with a single selected panel.
#[derive(Debug, Clone)]
pub struct Tabs {
    tabs: Vec<String>,
    selection: Selection<String>,
}

impl Tabs {
    /// Create tabs in display order and select the first tab, if present.
    /// Use a distinct name for each tab.
    pub fn new<I: IntoIterator<Item = S>, S: Into<String>>(tabs: I) -> Self {
        let tabs: Vec<String> = tabs.into_iter().map(Into::into).collect();
        let mut selection = Selection::new(SelectionMode::Single);
        if let Some(first) = tabs.first() {
            selection.select(first.clone());
        }
        Self { tabs, selection }
    }

    /// Borrow the selected tab name, or an empty string if there are no tabs.
    pub fn selected(&self) -> &str {
        self.selection.selected().first().map_or("", String::as_str)
    }

    /// Select the named tab and show its panel.
    ///
    /// # Errors
    /// Returns a rejection listing the tabs if the name is unknown.
    pub fn select(&mut self, tab: &str) -> Result<(), ActionError> {
        let Some(tab_name) = self.tabs.iter().find(|name| name.as_str() == tab) else {
            return Err(ActionError::Rejected(format!(
                "unknown tab `{tab}` (available: {:?})",
                self.tabs
            )));
        };
        self.selection.select(tab_name.clone());
        Ok(())
    }

    /// Borrow the tab names in display order.
    pub fn tabs(&self) -> &[String] {
        &self.tabs
    }

    /// Whether the named tab's panel is visible.
    pub fn is_visible(&self, tab: &str) -> bool {
        self.selection
            .selected()
            .first()
            .is_some_and(|name| name == tab)
    }

    /// Select the next tab, wrapping from the last tab to the first.
    pub fn next(&mut self) {
        if let Some(index) = self.selected_index() {
            self.selection
                .select(self.tabs[(index + 1) % self.tabs.len()].clone());
        }
    }

    /// Select the previous tab, wrapping from the first tab to the last.
    pub fn prev(&mut self) {
        if let Some(index) = self.selected_index() {
            let previous = index.checked_sub(1).unwrap_or(self.tabs.len() - 1);
            self.selection.select(self.tabs[previous].clone());
        }
    }

    fn selected_index(&self) -> Option<usize> {
        let selected = self.selection.selected().first()?;
        self.tabs.iter().position(|tab| tab == selected)
    }
}

const ACTIONS: [ActionSpec; 1] = [action("select", "Select a tab.", &[param("tab", "str")])];

impl Behavior for Tabs {
    fn describe(&self, out: &mut Describe) {
        out.value(self.selection.selected().first())
            .attr("tabs", &self.tabs);
    }

    fn actions(&self) -> Vec<ActionSpec> {
        ACTIONS.to_vec()
    }

    fn owns(&self, action: &str) -> bool {
        action == "select"
    }

    fn invoke(&mut self, name: &str, args: &Args) -> Option<Result<Value, ActionError>> {
        dispatch(&ACTIONS, name, args, || {
            let tab: String = args.get(0, "tab")?;
            self.select(&tab)
        })
    }
}
