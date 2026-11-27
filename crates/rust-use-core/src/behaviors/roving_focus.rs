use super::{action, dispatch, param};
use crate::{ActionError, ActionSpec, Args, Behavior, Describe, Value};

/// Layout used for interpreting arrow keys.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Orientation {
    /// Left and right move along the list.
    #[default]
    Horizontal,
    /// Up and down move along the list.
    Vertical,
    /// Arrows stay in their row or column, including in a partial final row.
    Grid {
        /// Number of columns. Zero is treated as one.
        columns: usize,
    },
}

/// Navigation keys accepted by roving focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Previous row.
    Up,
    /// Next row.
    Down,
    /// Previous column.
    Left,
    /// Next column.
    Right,
    /// First item.
    Home,
    /// Last item.
    End,
}

/// Keyboard navigation over indexed items. Focus starts unset.
///
/// ```
/// use rust_use_core::{Key,Orientation,RovingFocus};
/// let mut focus = RovingFocus::new(Orientation::Vertical, 3);
/// focus.handle_key(Key::End);
/// assert_eq!(focus.active(),Some(2));
/// ```
#[derive(Debug, Clone, Default)]
pub struct RovingFocus {
    /// Layout for directional navigation.
    pub orientation: Orientation,
    len: usize,
    active: Option<usize>,
    /// Cycle at the end of a row, column, or list.
    pub wrap: bool,
}

impl RovingFocus {
    /// Create a navigation group with wrapping disabled.
    pub fn new(orientation: Orientation, len: usize) -> Self {
        Self {
            orientation,
            len,
            active: None,
            wrap: false,
        }
    }

    /// Current focused index.
    pub fn active(&self) -> Option<usize> {
        self.active
    }

    /// Number of focusable items.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the group has no items.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Resize the group, clearing focus if its item disappeared.
    pub fn set_len(&mut self, len: usize) {
        self.len = len;
        self.active = self.active.filter(|&i| i < len);
    }

    /// Focus an index, returning false for out-of-range indices.
    pub fn focus(&mut self, i: usize) -> bool {
        if i >= self.len {
            return false;
        }
        self.active = Some(i);
        true
    }

    /// Focus the first item.
    pub fn first(&mut self) -> bool {
        self.focus(0)
    }

    /// Focus the last item.
    pub fn last(&mut self) -> bool {
        self.focus(self.len.saturating_sub(1))
    }

    fn step(&mut self, forward: bool, first: usize, last: usize, stride: usize) -> bool {
        let Some(i) = self.active else {
            return if forward { self.first() } else { self.last() };
        };
        let next = if forward {
            i.checked_add(stride).filter(|n| *n <= last)
        } else {
            i.checked_sub(stride).filter(|n| *n >= first)
        };
        let target = next.or_else(|| self.wrap.then_some(if forward { first } else { last }));
        target.is_some_and(|n| n != i && self.focus(n))
    }

    /// Move to the next item in display order.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> bool {
        self.step(true, 0, self.len.saturating_sub(1), 1)
    }

    /// Move to the preceding item in display order.
    pub fn prev(&mut self) -> bool {
        self.step(false, 0, self.len.saturating_sub(1), 1)
    }

    fn arrow(&mut self, vertical: bool, forward: bool) -> bool {
        let end = self.len.saturating_sub(1);
        match self.orientation {
            Orientation::Grid { columns } => {
                let cols = columns.max(1);
                let i = self.active.unwrap_or(0);
                if vertical {
                    let first = i % cols;
                    self.step(forward, first, end - (end - first) % cols, cols)
                } else {
                    let first = i - i % cols;
                    self.step(forward, first, first.saturating_add(cols - 1).min(end), 1)
                }
            }
            Orientation::Vertical if vertical => self.step(forward, 0, end, 1),
            Orientation::Horizontal if !vertical => self.step(forward, 0, end, 1),
            _ => false,
        }
    }

    /// Move up in the current column.
    pub fn up(&mut self) -> bool {
        self.arrow(true, false)
    }

    /// Move down in the current column.
    pub fn down(&mut self) -> bool {
        self.arrow(true, true)
    }

    /// Move left in the current row.
    pub fn left(&mut self) -> bool {
        self.arrow(false, false)
    }

    /// Move right in the current row.
    pub fn right(&mut self) -> bool {
        self.arrow(false, true)
    }

    /// Apply a navigation key, returning whether focus changed.
    pub fn handle_key(&mut self, key: Key) -> bool {
        let before = self.active;
        match key {
            Key::Up => self.up(),
            Key::Down => self.down(),
            Key::Left => self.left(),
            Key::Right => self.right(),
            Key::Home => self.first(),
            Key::End => self.last(),
        };
        before != self.active
    }
}

const ACTIONS: [ActionSpec; 1] = [action("focus", "Focus an item.", &[param("index", "int")])];

impl Behavior for RovingFocus {
    fn describe(&self, out: &mut Describe) {
        if let Some(i) = self.active {
            out.attr("focus", i);
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
            if self.focus(args.get(0, "index")?) {
                Ok(())
            } else {
                Err(ActionError::Rejected("focus index out of bounds".into()))
            }
        })
    }
}

#[test]
fn grid_edges() {
    let mut f = RovingFocus::new(Orientation::Grid { columns: 3 }, 8);
    f.focus(5);
    assert!(!f.right());
    assert!(!f.down());
    f.wrap = true;
    assert!(f.down());
    assert_eq!(f.active(), Some(2));
    assert!(f.up());
    assert_eq!(f.active(), Some(5));
    f.last();
    assert!(f.right());
    assert_eq!(f.active(), Some(6));
    assert!(f.left());
    assert_eq!(f.active(), Some(7));
    f.set_len(0);
    assert!(!f.down());
    assert_eq!(f.active(), None);
}
