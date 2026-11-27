use rust_use_macros::actions;

struct Invalid;

#[actions]
impl Invalid {
    /// Toggle.
    #[action(confirm = 42)]
    fn toggle(&mut self) {}
}

fn main() {}
