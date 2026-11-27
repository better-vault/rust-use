use rust_use_macros::Primitive;

#[derive(Primitive)]
#[primitive(unknown)]
struct Invalid {  field: bool }

fn main() {}
