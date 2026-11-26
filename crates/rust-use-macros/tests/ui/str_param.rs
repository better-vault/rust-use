use rust_use_macros::actions;

struct Invalid;

#[actions]
impl Invalid {
    /// Rename.
    fn rename(&mut self, name: &str) {}
}

fn main() {}
