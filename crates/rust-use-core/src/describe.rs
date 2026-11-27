use std::borrow::Cow;

use crate::Value;
use serde::Serialize;

/// Collects a primary value and named attributes in insertion order.
///
/// ```
/// use rust_use_core::Describe;
/// let mut out = Describe::new();
/// out.value(2).flag("selected", true);
/// assert_eq!(out.get("selected"), Some(&true.into()));
/// ```
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Describe {
    primary: Option<Value>,
    attrs: Vec<(Cow<'static, str>, Value)>,
}

impl Describe {
    /// Construct an empty description.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set or replace the primary value.
    pub fn value(&mut self, v: impl Serialize) -> &mut Self {
        self.primary = Some(serde_json::to_value(v).unwrap_or(Value::Null));
        self
    }

    /// Set a named attribute, preserving its first insertion position.
    pub fn attr(&mut self, key: impl Into<Cow<'static, str>>, v: impl Serialize) -> &mut Self {
        let key = key.into();
        let value = serde_json::to_value(v).unwrap_or(Value::Null);
        if let Some((_, old)) = self.attrs.iter_mut().find(|(k, _)| *k == key) {
            *old = value;
        } else {
            self.attrs.push((key, value));
        }
        self
    }

    /// Add a true attribute when the flag is on.
    pub fn flag(&mut self, key: impl Into<Cow<'static, str>>, on: bool) -> &mut Self {
        if on {
            self.attr(key, true);
        }
        self
    }

    /// Borrow the primary value, if set.
    pub fn primary(&self) -> Option<&Value> {
        self.primary.as_ref()
    }

    /// Iterate attributes in insertion order.
    pub fn attrs(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.attrs.iter().map(|(k, v)| (k.as_ref(), v))
    }

    /// Borrow a named attribute.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.attrs().find(|(k, _)| *k == key).map(|(_, v)| v)
    }
}
