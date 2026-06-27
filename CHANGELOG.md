# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- CI: remove unused `UnixStream` import on non-Windows IPC server
- CI: bump MSRV to Rust 1.86 for icu/idna dependency tree
- IPC: `FETCH` returns summary; `GET_PROFILE` redacts credentials and raw URIs
- IPC: `PING` includes protocol version and engine semver
- Docs: align IPC protocol, README, and config example with v0.1 implementation

## [0.1.0] - 2026-06-27

### Added

- Initial release of conflux-engine workspace
- `conflux-protocol` crate with `ConfluxNode`, `ConfluxSubscription`, and shared domain types
- `conflux-core` crate: HTTP subscription fetch, format detection, URI parsing, Clash YAML extraction, normalization pipeline
- `conflux-backend` crate: sing-box config generation and subprocess supervision
- `conflux-ipc` crate: JSON-over-line protocol v1 over Windows named pipe (`\\.\pipe\conflux-engine`)
- `conflux` CLI: `fetch`, `convert`, `validate`, `daemon` commands
- `confluxd` daemon: foreground IPC server with config from `CONFLUX_CONFIG`
- Root `conflux-engine` library facade re-exporting core and protocol
- Supported URI schemes: VLESS, VMess, Shadowsocks, Trojan, Hysteria2 (hy2 alias)
- Partial support for Clash YAML (`proxies[]`) and sing-box JSON outbounds
- JSON Schema for normalized profiles at `assets/schemas/normalized-profile.schema.json`
- Documentation: architecture, IPC protocol, subscription formats, sing-box backend, Windows integration
- CI workflow: fmt, clippy, test on Windows, Ubuntu, macOS
- Release workflow: semver tag builds for `conflux` and `confluxd` binaries
- Example: `examples/basic-fetch`
- Apache-2.0 license

[Unreleased]: https://github.com/spamulodd/conflux-engine/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/spamulodd/conflux-engine/releases/tag/v0.1.0
