//! Direct core dependency path resolution.
use rust_use_core::{Args, Behavior, Primitive as _, Value};
use rust_use_macros::{Primitive, actions};

#[derive(Primitive)]
struct HTTPStatus {
    code: u16,
}

#[actions]
impl HTTPStatus {
    /// Return the status.
    fn status(&mut self) -> u16 {
        self.code
    }
}

#[test]
fn direct_core_dependency() {
    let mut status = HTTPStatus { code: 200 };
    assert_eq!(HTTPStatus::NAME, "http-status");
    assert_eq!(
        status.invoke("status", &Args::new()),
        Some(Ok(Value::from(200)))
    );
}

#[derive(Primitive)]
#[primitive(crate = "::rust_use_core", no_actions)]
struct Indicator;

#[test]
fn primitive_without_actions() {
    assert!(Indicator.actions().is_empty());
    assert_eq!(Indicator.invoke("status", &Args::new()), None);
}
