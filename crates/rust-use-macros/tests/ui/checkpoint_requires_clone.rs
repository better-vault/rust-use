use rust_use_macros::Primitive;

#[derive(Primitive)]
#[primitive(no_actions, checkpoint)]
struct Invalid;

fn main() {}
