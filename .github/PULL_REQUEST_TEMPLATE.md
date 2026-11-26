<!--
Thanks for the pull request.

Keep it to one logical change — a PR that fixes a bug and refactors two other things is three
reviews wearing one coat. See CONTRIBUTING.md if you have not already.
-->

## What this changes

<!-- One or two sentences. What behaviour is different after this than before it? -->

## Why

<!--
The problem, not the patch. If there is an issue, link it — "Closes #123" will close it on merge.
-->

Closes #

## Type of change

<!-- Tick what applies. -->

- [ ] Bug fix
- [ ] New behavior
- [ ] New action, option or capability on an existing API
- [ ] Breaking change — an existing API is renamed, removed, or behaves differently
- [ ] Documentation only
- [ ] Internal: refactor, tooling, CI

## How it was tested

<!--
What did you actually run? "Ran the chess demo on macOS and played the Opera Game through the
CLI" beats "tested".
-->

- [ ] macOS
- [ ] Linux
- [ ] Windows

## Checklist

- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes
- [ ] `cargo test --workspace --all-features` passes
- [ ] Commits follow [Conventional Commits](https://www.conventionalcommits.org/)
- [ ] Every new public item has a doc comment, and an example where it is not obvious
- [ ] No `unwrap`/`expect` in library code — errors are typed and returned

### If this touches what an agent sees

- [ ] The snapshot format spec in `crates/rust-use-agent/FORMAT.md` is updated
- [ ] The snapshot size tests still pass, or the new budget is justified below

## Notes for the reviewer

<!--
Anything you are unsure about, a decision you would like a second opinion on, or a part of the
diff worth reading first.
-->
