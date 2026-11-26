use rust_use_macros::Primitive;

#[derive(Primitive)]
struct Invalid {
    #[agent(value)] first: u8,
    #[agent(value)] second: u8,
}

fn main() {}
