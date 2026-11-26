use rust_use_macros::actions;

struct Invalid;

#[actions]
impl Invalid {
    /// Set.
    fn set<T>(&mut self, value: T) {}
}

fn main() {}
