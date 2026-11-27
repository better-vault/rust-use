mod drag_drop;

mod list;

mod popup;

mod press;

mod roving_focus;

mod selection;

mod toggle;

mod transfer;

mod tree;

mod typeahead;

mod value;

pub use drag_drop::DragDrop;
pub use list::List;
pub use popup::Popup;
pub use press::Press;
pub use roving_focus::{Key, Orientation, RovingFocus};
pub use selection::{Selection, SelectionMode};
pub use toggle::Toggle;
pub use transfer::Transfer;
pub use tree::{Tree, TreeRow};
pub use typeahead::Typeahead;
pub use value::Field;

use crate::{ActionError, ActionSpec, Args, ParamSpec, Value};

const fn param(name: &'static str, ty: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty,
        optional: false,
    }
}

const fn param_opt(name: &'static str, ty: &'static str) -> ParamSpec {
    ParamSpec {
        optional: true,
        ..param(name, ty)
    }
}

const fn action(name: &'static str, doc: &'static str, params: &'static [ParamSpec]) -> ActionSpec {
    ActionSpec { name, doc, params }
}

fn dispatch(
    specs: &[ActionSpec],
    name: &str,
    args: &Args,
    apply: impl FnOnce() -> Result<(), ActionError>,
) -> Option<Result<Value, ActionError>> {
    let spec = specs.iter().find(|spec| spec.name == name)?;
    Some(if args.len() > spec.params.len() {
        Err(ActionError::InvalidArg {
            name: "args",
            reason: format!("expected {}, got {}", spec.params.len(), args.len()),
        })
    } else {
        apply().map(|()| Value::Null)
    })
}
