use rust_use_macros::Primitive;

#[derive(Primitive)]
struct Invalid { #[agent(flag, secret)] field: bool }

fn main() {}
