use rust_use_macros::Primitive;

#[derive(Primitive)]
struct Invalid { #[agent(hidden)] field: bool }

fn main() {}
