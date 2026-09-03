## Description
<!-- Briefly describe the changes introduced by this pull request. -->

## Type of Change
- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] Documentation update
- [ ] Code hygiene / refactoring (no behavioral changes)
- [ ] Profile / network configuration update

## Verification Checklist
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes without warnings
- [ ] `cargo test --workspace` passes all tests
- [ ] If changing network profiles or packet parameters: physical test evidence on real Turkish hardware attached (ISP, city, `curl -v` log)
- [ ] No secrets, tokens, or local path leaks introduced
- [ ] Upstream licenses and copyright notices preserved
