## Description

<!-- What does this PR change, and why? Link any related issues. -->

## Checklist

- [ ] I've described the change and the motivation above
- [ ] Tests were added or updated to cover this change (or N/A, with reason)
- [ ] `cargo fmt` passes with no diff
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace` passes
- [ ] All `.wipe` writes still go through `wipe-core`'s deterministic
      serializer (no ad-hoc JSON serialization added)
- [ ] The PR title follows [Conventional Commits](https://www.conventionalcommits.org/)
      (e.g. `feat(cli): add wipe ticket archive command`)
