# Contributing to conflux-engine

Thank you for your interest in contributing. This project welcomes bug fixes, documentation improvements, parser additions, and test fixtures.

## Getting Started

### Prerequisites

- Rust **1.80+** (see `rust-toolchain.toml`)
- `cargo fmt`, `cargo clippy`, `cargo test`
- Optional: sing-box binary for backend integration tests

### Setup

```bash
git clone https://github.com/your-org/conflux-engine.git
cd conflux-engine
cargo build --workspace
cargo test --workspace
```

## Development Workflow

1. Fork the repository and create a feature branch from `main`
2. Make your changes with tests where applicable
3. Run the full check suite locally:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo fmt --all -- --check
```

4. Open a pull request with a clear description of the change

## Code Guidelines

- **English only** for code comments, documentation, and commit messages
- Match existing naming and module structure in each crate
- Keep changes focused; avoid unrelated refactors in the same PR
- Add unit tests with inline fixtures for parser changes
- **Never commit real subscription URLs, tokens, or live credentials** in tests or docs

### Parser contributions

When adding a new subscription format:

1. Add detection heuristic to `conflux-core/src/parse/`
2. Map output to `ConfluxNode` in `normalize`
3. Add redacted inline test fixtures
4. Document the format in `docs/subscription-formats.md`
5. Update the supported formats table in `README.md`

### IPC changes

Protocol changes require:

1. Version increment in `conflux-ipc/src/protocol.rs`
2. Documentation update in `docs/ipc-protocol.md`
3. Backward compatibility note in `CHANGELOG.md`

## Commit Messages

Use clear, imperative subject lines:

```
Add trojan URI parser for legacy query format
Fix Base64 padding for URL-safe alphabet
Document STATUS response fields in ipc-protocol.md
```

## Pull Request Checklist

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace` passes
- [ ] Documentation updated for user-facing changes
- [ ] CHANGELOG.md updated under `[Unreleased]` for notable changes
- [ ] No secrets or live subscription tokens in the diff

## License

By contributing, you agree that your contributions will be licensed under the [Apache License 2.0](LICENSE).
