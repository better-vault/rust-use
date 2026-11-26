use super::{action, dispatch, param};
use crate::{ActionError, ActionSpec, Args, Behavior, Describe, Value};
use serde::{Serialize, de::DeserializeOwned};

/// Number of keys a selection can hold.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    /// Select at most one key.
    #[default]
    Single,
    /// Select distinct keys in insertion order.
    Multiple,
}
/// Ordered selection of item keys.
///
/// ```
/// use rust_use_core::{Selection,SelectionMode};
/// let mut choices = Selection::new(SelectionMode::Single);
/// choices.select("blue");
/// assert!(choices.is_selected(&"blue"));
/// ```
#[derive(Debug, Clone)]
pub struct Selection<K> {
    mode: SelectionMode,
    keys: Vec<K>,
}
impl<K> Default for Selection<K> {
    fn default() -> Self {
        Self::new(SelectionMode::Single)
    }
}
impl<K> Selection<K> {
    /// Create an empty selection.
    pub fn new(mode: SelectionMode) -> Self {
        Self {
            mode,
            keys: Vec::new(),
        }
    }
    /// Read the selection policy.
    pub fn mode(&self) -> SelectionMode {
        self.mode
    }
    /// Borrow selected keys in selection order.
    pub fn selected(&self) -> &[K] {
        &self.keys
    }
    /// Remove all keys.
    pub fn clear(&mut self) {
        self.keys.clear();
    }
}
impl<K: Eq> Selection<K> {
    /// Test membership.
    pub fn is_selected(&self, key: &K) -> bool {
        self.keys.contains(key)
    }
    /// Select a key, returning whether the selection changed.
    pub fn select(&mut self, key: K) -> bool {
        if self.is_selected(&key) {
            return false;
        }
        if self.mode == SelectionMode::Single {
            self.clear();
        }
        self.keys.push(key);
        true
    }
    /// Remove a key, returning whether it was selected.
    pub fn deselect(&mut self, key: &K) -> bool {
        if let Some(i) = self.keys.iter().position(|k| k == key) {
            self.keys.remove(i);
            true
        } else {
            false
        }
    }
    /// Toggle membership and return the resulting membership.
    pub fn toggle(&mut self, key: K) -> bool {
        !self.deselect(&key) && self.select(key)
    }
}
const ACTIONS: [ActionSpec; 3] = [
    action("select", "Select a key.", &[param("key", "any")]),
    action("deselect", "Remove a key.", &[param("key", "any")]),
    action("clear", "Clear the selection.", &[]),
];
impl<K: Serialize + DeserializeOwned + Eq + Clone + Send + 'static> Behavior for Selection<K> {
    fn describe(&self, out: &mut Describe) {
        if let Some(first) = self.keys.first() {
            match self.mode {
                SelectionMode::Single => out.attr("sel", first),
                SelectionMode::Multiple => out.attr("sel", &self.keys),
            };
        }
    }
    fn actions(&self) -> Vec<ActionSpec> {
        ACTIONS.to_vec()
    }
    fn is_enabled(&self, name: &str) -> bool {
        match name {
            "select" => true,
            "clear" => !self.keys.is_empty(),
            "deselect" => self.mode == SelectionMode::Multiple,
            _ => false,
        }
    }
    fn invoke(&mut self, name: &str, args: &Args) -> Option<Result<Value, ActionError>> {
        dispatch(&ACTIONS, name, args, || {
            if !self.is_enabled(name) {
                return Err(ActionError::Disabled(name.into()));
            }
            match name {
                "select" => {
                    self.select(args.get(0, "key")?);
                }
                "deselect" => {
                    self.deselect(&args.get(0, "key")?);
                }
                _ => self.clear(),
            }
            Ok(())
        })
    }
}
#[test]
fn single_replaces() {
    let mut s = Selection::default();
    s.select(1);
    s.select(2);
    assert_eq!(s.selected(), &[2]);
}
