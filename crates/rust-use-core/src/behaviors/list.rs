use super::{action, dispatch, param};
use crate::{ActionError, ActionSpec, Args, Behavior, Describe, Page, Query, Value};
use serde::Serialize;
use std::fmt;

type Filter<T> = Box<dyn Fn(&T, &str) -> bool + Send>;

/// An ordered, searchable list.
///
/// ```
/// use rust_use_core::List;
/// let mut items = List::new(vec!["one"]);
/// items.push("two");
/// assert_eq!(items.remove(0), Some("one"));
/// ```
pub struct List<T> {
    /// Items in display order.
    pub items: Vec<T>,
    /// Optional predicate receiving the original query text.
    pub filter: Option<Filter<T>>,
}
impl<T: fmt::Debug> fmt::Debug for List<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("List")
            .field("items", &self.items)
            .finish_non_exhaustive()
    }
}
impl<T> Default for List<T> {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}
impl<T> List<T> {
    /// Create a list in display order.
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items,
            filter: None,
        }
    }
    /// Install a custom text filter.
    #[must_use]
    pub fn with_filter(mut self, f: impl Fn(&T, &str) -> bool + Send + 'static) -> Self {
        self.filter = Some(Box::new(f));
        self
    }
    /// Append an item.
    pub fn push(&mut self, item: T) {
        self.items.push(item);
    }
    /// Remove the item at an index, if present.
    pub fn remove(&mut self, i: usize) -> Option<T> {
        (i < self.len()).then(|| self.items.remove(i))
    }
    /// Move an item to its final index.
    /// # Errors
    /// Returns a rejection if either index is out of bounds.
    pub fn move_item(&mut self, from: usize, to: usize) -> Result<(), ActionError> {
        if from >= self.len() || to >= self.len() {
            return Err(ActionError::Rejected("list index out of bounds".into()));
        }
        let item = self.items.remove(from);
        self.items.insert(to, item);
        Ok(())
    }
    /// Number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }
    /// Whether there are no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    /// Iterate items in display order.
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.items.iter()
    }
}
impl<'a, T> IntoIterator for &'a List<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
const ACTIONS: [ActionSpec; 1] = [action(
    "move",
    "Reorder an item.",
    &[param("from", "int"), param("to", "int")],
)];
impl<T: Serialize + Send + 'static> Behavior for List<T> {
    fn describe(&self, out: &mut Describe) {
        out.attr("n", self.len());
    }
    fn actions(&self) -> Vec<ActionSpec> {
        ACTIONS.to_vec()
    }
    fn invoke(&mut self, name: &str, args: &Args) -> Option<Result<Value, ActionError>> {
        dispatch(&ACTIONS, name, args, || {
            self.move_item(args.get(0, "from")?, args.get(1, "to")?)
        })
    }
    fn query(&self, q: &Query) -> Option<Page> {
        let needle = q.text.as_deref().map(str::to_lowercase);
        let items = self.iter().filter_map(|item| {
            if let (Some(text), Some(filter)) = (&q.text, &self.filter)
                && !filter(item, text)
            {
                return None;
            }
            let value = serde_json::to_value(item).unwrap_or(Value::Null);
            if let (None, Some(needle)) = (&self.filter, &needle)
                && !value.to_string().to_lowercase().contains(needle)
            {
                return None;
            }
            Some(value)
        });
        Some(Page::from_iter(items, q))
    }
}
#[test]
fn bounds_are_atomic() {
    let mut list = List::new(vec![0, 1]);
    for (from, to) in [(0, 2), (2, 0)] {
        assert!(list.move_item(from, to).is_err());
        assert_eq!(list.items, [0, 1]);
    }
}
