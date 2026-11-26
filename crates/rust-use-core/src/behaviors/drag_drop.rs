use super::{action, dispatch, param};
use crate::{ActionError, ActionSpec, Args, Behavior, Describe, Value};
use serde::{Serialize, de::DeserializeOwned};
use std::fmt;

type Handler<Id> = Box<dyn FnMut(Id, Id) -> Result<(), String> + Send>;

/// Tracks pointer drags and accepts atomic agent drops.
///
/// ```
/// use rust_use_core::DragDrop;
/// let mut drag = DragDrop::default();
/// drag.start(1);
/// assert_eq!(drag.drop_on(2),Some((1,2)));
/// ```
pub struct DragDrop<Id> {
    source: Option<Id>,
    target: Option<Id>,
    handler: Option<Handler<Id>>,
}
impl<Id: fmt::Debug> fmt::Debug for DragDrop<Id> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DragDrop")
            .field("source", &self.source)
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}
impl<Id> Default for DragDrop<Id> {
    fn default() -> Self {
        Self {
            source: None,
            target: None,
            handler: None,
        }
    }
}
impl<Id> DragDrop<Id> {
    /// Set the handler called before accepting a drop.
    #[must_use]
    pub fn on_drop(mut self, f: impl FnMut(Id, Id) -> Result<(), String> + Send + 'static) -> Self {
        self.handler = Some(Box::new(f));
        self
    }
    /// Begin a drag, resetting the hover target.
    pub fn start(&mut self, id: Id) {
        self.source = Some(id);
        self.target = None;
    }
    /// Update the hover target.
    pub fn over(&mut self, id: Option<Id>) {
        self.target = id;
    }
    /// End a pointer drag without dropping.
    pub fn cancel(&mut self) {
        self.source = None;
        self.target = None;
    }
    /// Current pointer drag source.
    pub fn dragging(&self) -> Option<&Id> {
        self.source.as_ref()
    }
    fn notify(&mut self, from: Id, to: Id) -> Result<(), ActionError> {
        if let Some(handler) = &mut self.handler {
            handler(from, to)?;
        }
        Ok(())
    }
}
impl<Id: Clone> DragDrop<Id> {
    /// Complete a pointer drop. An absent source or rejected drop yields None.
    pub fn drop_on(&mut self, to: Id) -> Option<(Id, Id)> {
        let from = self.source.as_ref()?.clone();
        self.notify(from, to.clone()).ok()?;
        self.target = None;
        self.source.take().map(|id| (id, to))
    }
}
const ACTIONS: [ActionSpec; 1] = [action(
    "drop",
    "Drop a source on a target.",
    &[param("from", "any"), param("to", "any")],
)];
impl<Id: Serialize + DeserializeOwned + Clone + Send + 'static> Behavior for DragDrop<Id> {
    fn describe(&self, out: &mut Describe) {
        if let Some(id) = self.dragging() {
            out.attr("drag", id);
        }
    }
    fn actions(&self) -> Vec<ActionSpec> {
        ACTIONS.to_vec()
    }
    fn invoke(&mut self, name: &str, args: &Args) -> Option<Result<Value, ActionError>> {
        dispatch(&ACTIONS, name, args, || {
            self.notify(args.get(0, "from")?, args.get(1, "to")?)
        })
    }
}
#[test]
fn rejection_preserves_pointer_drag() {
    let mut drag = DragDrop::default().on_drop(|_, _| Err("occupied".into()));
    drag.start(7);
    drag.over(Some(8));
    assert_eq!(drag.drop_on(9), None);
    assert_eq!(
        drag.invoke("drop", &Args::from((1, 2))),
        Some(Err(ActionError::Rejected("occupied".into())))
    );
    assert_eq!(drag.dragging(), Some(&7));
    assert_eq!(drag.target, Some(8));
}
