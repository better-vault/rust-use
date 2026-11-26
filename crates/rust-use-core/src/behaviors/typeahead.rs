use crate::{Behavior, Describe};
use std::time::{Duration, Instant};

/// Prefix search with repeated-character cycling.
///
/// ```
/// use rust_use_core::Typeahead;
/// use std::time::Instant;
/// let mut search = Typeahead::default();
/// assert_eq!(search.push_char('r', Instant::now(), &["Blue","Red"]), Some(1));
/// ```
#[derive(Debug, Clone)]
pub struct Typeahead {
    buffer: String,
    last_input: Option<Instant>,
    matched: Option<usize>,
    timeout: Duration,
}
impl Default for Typeahead {
    fn default() -> Self {
        Self::new(Duration::from_millis(500))
    }
}
impl Typeahead {
    /// Start an empty search with the specified inactivity timeout.
    pub fn new(timeout: Duration) -> Self {
        Self {
            buffer: String::new(),
            last_input: None,
            matched: None,
            timeout,
        }
    }
    /// Search labels in display order. Repeating a single character cycles through matches.
    pub fn push_char(&mut self, c: char, now: Instant, labels: &[&str]) -> Option<usize> {
        if self
            .last_input
            .and_then(|last| now.checked_duration_since(last))
            .is_none_or(|age| age >= self.timeout)
        {
            self.buffer.clear();
            self.matched = None;
        }
        self.last_input = Some(now);
        let token = c.to_lowercase().to_string();
        let start = if self.buffer == token {
            self.matched.map_or(0, |i| (i + 1).min(labels.len()))
        } else {
            self.buffer.push_str(&token);
            0
        };
        self.matched = (start..labels.len())
            .chain(0..start)
            .find(|&i| labels[i].to_lowercase().starts_with(&self.buffer));
        self.matched
    }
}
impl Behavior for Typeahead {
    fn describe(&self, _: &mut Describe) {}
}
#[test]
fn cycles_then_resets() {
    let mut t = Typeahead::default();
    let now = Instant::now();
    let labels = ["Bee", "Bat", "Box"];
    for i in [0, 1, 2, 0] {
        assert_eq!(t.push_char('B', now, &labels), Some(i));
    }
    assert_eq!(t.push_char('a', now, &labels), Some(1));
    assert_eq!(
        t.push_char('b', now + Duration::from_millis(500), &labels),
        Some(0)
    );
}
