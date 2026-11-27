use super::{action, dispatch, param, param_opt};
use crate::{ActionError, ActionSpec, Args, Behavior, Describe, Page, Query, Value};
use serde::{Serialize, de::DeserializeOwned};
use std::{borrow::Cow, fmt};

type Accept<T> = Box<dyn Fn(&T, &str) -> bool + Send>;

/// Items arranged in named lists, moved within and between them.
///
/// An agent can query a named list with `<list>` or narrow it with
/// `<list> <filter>` using a case-insensitive substring. Without query text,
/// the page contains `"<list>: <item>"` strings across all lists.
pub struct Transfer<T> {
    lists: Vec<(String, Vec<T>)>,
    accept: Option<Accept<T>>,
}

impl<T: fmt::Debug> fmt::Debug for Transfer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transfer")
            .field("lists", &self.lists)
            .finish_non_exhaustive()
    }
}

impl<T> Default for Transfer<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Transfer<T> {
    /// Create a transfer with no lists.
    pub fn new() -> Self {
        Self {
            lists: Vec::new(),
            accept: None,
        }
    }

    /// Add a list, or replace an existing list's items in place.
    #[must_use]
    pub fn with_list(mut self, name: impl Into<String>, items: Vec<T>) -> Self {
        let name = name.into();
        if let Some(index) = self.list_index(&name) {
            self.lists[index].1 = items;
        } else {
            self.lists.push((name, items));
        }
        self
    }

    /// Install a predicate that must accept the item and destination for a move.
    #[must_use]
    pub fn accept(mut self, f: impl Fn(&T, &str) -> bool + Send + 'static) -> Self {
        self.accept = Some(Box::new(f));
        self
    }

    /// Iterate list names and items in list order.
    pub fn lists(&self) -> impl Iterator<Item = (&str, &[T])> {
        self.lists
            .iter()
            .map(|(name, items)| (name.as_str(), items.as_slice()))
    }

    /// Borrow the items of a named list.
    pub fn list(&self, name: &str) -> Option<&[T]> {
        self.list_index(name)
            .map(|index| self.lists[index].1.as_slice())
    }

    /// Move the item at `index` in `from` to its final index `at` in `to`.
    /// Pass `None` to append it.
    ///
    /// # Errors
    /// Rejects unknown lists, out-of-range indices, and moves refused by the predicate.
    pub fn move_item(
        &mut self,
        from: &str,
        index: usize,
        to: &str,
        at: Option<usize>,
    ) -> Result<(), ActionError> {
        let from = self
            .list_index(from)
            .ok_or_else(|| self.unknown_list(from))?;
        let to = self.list_index(to).ok_or_else(|| self.unknown_list(to))?;
        self.move_between(from, index, to, at)
    }

    fn list_index(&self, name: &str) -> Option<usize> {
        self.lists.iter().position(|(key, _)| key == name)
    }

    fn unknown_list(&self, name: &str) -> ActionError {
        let names = self.lists().map(|(name, _)| name).collect::<Vec<_>>();
        ActionError::Rejected(format!("unknown list {name:?}; available lists: {names:?}"))
    }

    fn move_between(
        &mut self,
        from: usize,
        index: usize,
        to: usize,
        at: Option<usize>,
    ) -> Result<(), ActionError> {
        let (source, items) = &self.lists[from];
        let item = items.get(index).ok_or_else(|| {
            ActionError::Rejected(format!(
                "item index {index} out of range for list {source:?}"
            ))
        })?;
        let (destination, items) = &self.lists[to];
        let end = items.len() - usize::from(from == to);
        let at = at.unwrap_or(end);
        if at > end {
            return Err(ActionError::Rejected(format!(
                "destination index {at} out of range for list {destination:?} (expected 0..={end})"
            )));
        }
        if self
            .accept
            .as_ref()
            .is_some_and(|accept| !accept(item, destination))
        {
            return Err(ActionError::Rejected(format!(
                "list {destination:?} refused the item"
            )));
        }
        let item = self.lists[from].1.remove(index);
        self.lists[to].1.insert(at, item);
        Ok(())
    }
}

impl<T: PartialEq> Transfer<T> {
    /// Find the first matching item in list and item order, returning its list name and index.
    pub fn find(&self, item: &T) -> Option<(&str, usize)> {
        let (list, index) = self.find_position(item)?;
        Some((&self.lists[list].0, index))
    }

    fn find_position(&self, item: &T) -> Option<(usize, usize)> {
        self.lists
            .iter()
            .enumerate()
            .find_map(|(list, (_, items))| {
                items
                    .iter()
                    .position(|value| value == item)
                    .map(|index| (list, index))
            })
    }
}

const ACTIONS: [ActionSpec; 1] = [action(
    "move",
    "Move an item by value to a named list.",
    &[
        param("item", "any"),
        param("to", "str"),
        param_opt("at", "int"),
    ],
)];

impl<T: Serialize + DeserializeOwned + PartialEq + Send + 'static> Behavior for Transfer<T> {
    fn describe(&self, out: &mut Describe) {
        for (name, items) in self.lists() {
            out.attr(name.to_owned(), items.len());
        }
    }

    fn actions(&self) -> Vec<ActionSpec> {
        ACTIONS.to_vec()
    }

    fn owns(&self, action: &str) -> bool {
        ACTIONS.iter().any(|a| a.name == action)
    }

    fn invoke(&mut self, name: &str, args: &Args) -> Option<Result<Value, ActionError>> {
        dispatch(&ACTIONS, name, args, || {
            let item = args.get(0, "item")?;
            let to: String = args.get(1, "to")?;
            let at = args
                .get_opt::<i128>(2, "at")?
                .map(|at| {
                    usize::try_from(at).map_err(|_| {
                        ActionError::Rejected(format!("destination index {at} out of range"))
                    })
                })
                .transpose()?;
            let to = self.list_index(&to).ok_or_else(|| self.unknown_list(&to))?;
            let (from, index) = self.find_position(&item).ok_or_else(|| {
                ActionError::Rejected("item not found; query the lists for items".into())
            })?;
            self.move_between(from, index, to, at)
        })
    }

    fn queryable(&self) -> bool {
        true
    }

    fn query(&self, q: &Query) -> Option<Page> {
        Some(match q.text.as_deref() {
            Some(text) => self.query_list(text, q),
            None => self.query_all(q),
        })
    }
}

impl<T: Serialize> Transfer<T> {
    fn query_list(&self, text: &str, q: &Query) -> Page {
        let matched = self
            .lists()
            .filter_map(|(name, items)| {
                let suffix = text.strip_prefix(name)?;
                (suffix.is_empty() || suffix.starts_with(char::is_whitespace)).then_some((
                    name,
                    items,
                    suffix.trim(),
                ))
            })
            .max_by_key(|(name, _, _)| name.len());
        let Some((_, items, filter)) = matched else {
            return Page::default();
        };
        if filter.is_empty() {
            return unfiltered_page(items.iter(), items.len(), q, item_value);
        }
        let needle = filter.to_lowercase();
        let items = items
            .iter()
            .map(item_value)
            .filter(|value| item_text(value).to_lowercase().contains(&needle));
        Page::from_iter(items, q)
    }

    fn query_all(&self, q: &Query) -> Page {
        let total = self.lists().map(|(_, items)| items.len()).sum();
        let items = self
            .lists()
            .flat_map(|(name, items)| items.iter().map(move |item| (name, item)));
        unfiltered_page(items, total, q, |(name, item)| {
            let value = item_value(item);
            Value::String(format!("{name}: {}", item_text(&value)))
        })
    }
}

fn unfiltered_page<T>(
    items: impl Iterator<Item = T>,
    total: usize,
    q: &Query,
    encode: impl FnMut(T) -> Value,
) -> Page {
    let start = q.from.min(total);
    let end = start.saturating_add(q.effective_limit()).min(total);
    let items = items.skip(start).take(end - start).map(encode).collect();
    Page {
        items,
        total,
        next: (end < total).then_some(end),
    }
}

fn item_value(item: &impl Serialize) -> Value {
    serde_json::to_value(item).unwrap_or(Value::Null)
}

fn item_text(value: &Value) -> Cow<'_, str> {
    value
        .as_str()
        .map_or_else(|| Cow::Owned(value.to_string()), Cow::Borrowed)
}

#[test]
fn moves_within_and_between_lists() -> Result<(), ActionError> {
    let mut transfer = Transfer::new()
        .with_list("todo", vec![0])
        .with_list("done", vec![4])
        .with_list("todo", vec![1, 2, 3]);

    transfer.move_item("todo", 0, "todo", Some(2))?;
    assert_eq!(transfer.list("todo"), Some([2, 3, 1].as_slice()));
    transfer.move_item("todo", 0, "todo", None)?;
    assert_eq!(transfer.list("todo"), Some([3, 1, 2].as_slice()));
    transfer.move_item("todo", 1, "done", Some(0))?;
    transfer.move_item("todo", 0, "done", Some(2))?;
    assert_eq!(
        transfer.lists().collect::<Vec<_>>(),
        [("todo", [2].as_slice()), ("done", [1, 4, 3].as_slice())]
    );
    assert_eq!(transfer.find(&3), Some(("done", 2)));
    assert_eq!(transfer.find(&0), None);
    Ok(())
}

#[test]
fn invalid_indices_and_vetoes_leave_lists_unchanged() {
    let mut transfer = Transfer::new()
        .with_list("todo", vec![1])
        .with_list("done", vec![2])
        .accept(|_, to| to != "done");
    for (from, index, to, at, reason) in [
        ("todo", 1, "todo", None, "out of range"),
        ("todo", 0, "todo", Some(1), "out of range"),
        ("todo", 0, "done", Some(2), "out of range"),
        ("todo", 0, "done", Some(1), "refused"),
        ("todo", 0, "missing", None, r#"["todo", "done"]"#),
    ] {
        assert!(matches!(
            transfer.move_item(from, index, to, at),
            Err(ActionError::Rejected(message)) if message.contains(reason)
        ));
        assert_eq!(transfer.list("todo"), Some([1].as_slice()));
        assert_eq!(transfer.list("done"), Some([2].as_slice()));
    }
}

#[test]
fn queries_page_across_lists_or_within_a_named_list() {
    let transfer = Transfer::new()
        .with_list(
            "todo",
            ["Fix login", "Write tests"].map(str::to_owned).into(),
        )
        .with_list(
            "in review",
            ["Fix docs", "Add examples"].map(str::to_owned).into(),
        );
    for (text, from, expected, total, next) in [
        (None, 1, "todo: Write tests", 4, Some(2)),
        (None, 2, "in review: Fix docs", 4, Some(3)),
        (Some("in review"), 1, "Add examples", 2, None),
        (Some("todo FIX"), 0, "Fix login", 1, None),
    ] {
        let q = Query {
            text: text.map(str::to_owned),
            from,
            limit: 1,
        };
        assert_eq!(
            transfer.query(&q),
            Some(Page {
                items: vec![expected.into()],
                total,
                next,
            })
        );
    }
}
