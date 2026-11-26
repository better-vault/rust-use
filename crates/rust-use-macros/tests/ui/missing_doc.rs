use rust_use_macros::actions;

struct Invalid;

#[actions]
impl Invalid {
    fn toggle(&mut self) {}
}

fn main() {}
