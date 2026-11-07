## Summary

<!-- What changes, and why. -->

## Testing

- [ ] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass
- [ ] Touched `install.sh`: `tests/install-e2e.sh target/debug` passes
- [ ] Touched an app's steps in `crates/core/src/catalog.rs`: ran that install, update and uninstall for real, and say which below

## Checklist

- [ ] `--json` output unchanged, or the change is in CHANGELOG.md and `docs/json-api.md`
- [ ] Changed the window: regenerated `docs/images` (`cargo test -p vinopilot-gui --test screenshots -- --ignored`)
- [ ] Comments explain *why*, not what, and any measured claim carries a date
- [ ] No secrets, tokens or machine-specific paths committed
