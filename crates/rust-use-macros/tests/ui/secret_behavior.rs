use rust_use_macros::Primitive;

#[derive(Primitive)]
struct Invalid { #[agent(secret)] #[behavior] field: bool }

fn main() {}
