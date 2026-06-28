# conflux-engine

[![CI](https://github.com/spamulodd/conflux-engine/actions/workflows/ci.yml/badge.svg)](https://github.com/spamulodd/conflux-engine/actions/workflows/ci.yml)

Universal subscription fetch, parse, and normalize engine for proxy and VPN clients. Conflux-engine downloads provider subscriptions, detects their format, maps nodes to a canonical profile model, and ships a [sing-box](https://sing-box.sagernet.org/) config generator for the data plane. It is designed for integration with desktop VPN clients via a JSON-line IPC daemon.

## Features

- **HTTP subscription fetch** — TLS via rustls; parses standard subscription headers (`Profile-Title`, `Subscription-Userinfo`, `Profile-Update-Interval`, `Announce`, `Support-Url`); **`happ://crypt*` links** (v0.2) via bundled `happ-decrypt.exe` helper
- **Multi-format parsing** — Base64 URI lists, plaintext URI lines, Clash YAML (`proxies[]`)
- **Protocol coverage (v0.1)** — VLESS, VMess, Shadowsocks, Trojan, Hysteria2 (`hy2` alias), native `himera://` tunnel URIs
- **Normalized profile model** — backend-agnostic `ConfluxSubscription` / `ConfluxNode` types with a published JSON Schema
- **sing-box backend library** — translate normalized profiles to sing-box JSON and supervise a subprocess (used from Rust; daemon `CONNECT`/`DISCONNECT` IPC in v0.2.1)
- **Windows IPC** — JSON envelope protocol v1 over `\\.\pipe\conflux-engine` (Unix socket for CI/dev on Linux/macOS)
- **CLI tools** — `conflux` for fetch/convert/validate; `confluxd` for IPC daemon mode

## Quick Start

### Prerequisites

- Rust **1.86+** (see `rust-toolchain.toml`)
- Optional: [sing-box](https://sing-box.sagernet.org/) binary for backend library tests (see [docs/backend-singbox.md](docs/backend-singbox.md))

### Install from source

```bash
git clone https://github.com/spamulodd/conflux-engine.git
cd conflux-engine
cargo build --release
```

Binaries: `target/release/conflux` and `target/release/confluxd`.

### CLI examples

```bash
conflux fetch https://example.com/sub/token --output profile.json
conflux convert subscription.txt --format auto --output profile.json
conflux validate profile.json
confluxd   # IPC on \\.\pipe\conflux-engine (Windows)
```

### Library usage

```rust
use conflux_engine::fetch_and_normalize;

let subscription = fetch_and_normalize("https://example.com/sub/token").await?;
```

See [examples/basic-fetch/src/main.rs](examples/basic-fetch/src/main.rs).

## Architecture

```
┌─────────────────────┐     named pipe      ┌──────────────────┐
│  Desktop VPN client │ ◄──────────────────►│   confluxd       │
│  (UI / coordinator) │   JSON line IPC     │   (daemon)       │
└─────────────────────┘                     └────────┬─────────┘
                                                     │
                                            ┌────────▼─────────┐
                                            │  conflux-core    │
                                            │  fetch → parse   │
                                            │  → normalize     │
                                            └────────┬─────────┘
                                                     │
                                            ┌────────▼─────────┐
                                            │ conflux-backend  │
                                            │ (sing-box cfg)   │
                                            └────────┬─────────┘
                                                     │
                                            ┌────────▼─────────┐
                                            │    sing-box      │
                                            │  (subprocess)    │
                                            └──────────────────┘
```

| Crate | Role |
|-------|------|
| `conflux-protocol` | Shared types: `ConfluxNode`, `ConfluxSubscription`, errors |
| `conflux-core` | Fetch, parse, normalize pipeline |
| `conflux-backend` | Backend trait; sing-box config generation and process management |
| `conflux-ipc` | IPC server and client |
| `conflux-cli` | `conflux` binary |
| `conflux-daemon` | `confluxd` binary |
| `conflux-engine` | Root library facade |

Detailed design: [docs/architecture.md](docs/architecture.md)

## Supported Subscription Formats

| Format | v0.1.0 status |
|--------|---------------|
| Base64-encoded URI list | Supported |
| Plaintext URI list | Supported |
| VLESS / VMess / SS / Trojan / Hysteria2 URI | Supported |
| Clash / Mihomo YAML (`proxies[]`) | Partial |
| Happ `#profile-title:` body directives | Metadata overlay |
| sing-box JSON / Xray JSON / TUIC / WireGuard URI | Planned |

Full heuristics: [docs/subscription-formats.md](docs/subscription-formats.md)

## Windows IPC Integration

Clients send JSON request envelopes; the daemon replies with `{"v":1,"status":"OK","data":{...}}` or `{"v":1,"status":"ERR","msg":"..."}`.

| Command | v0.2 behavior |
|---------|---------------|
| `PING` | Health + protocol/engine version |
| `FETCH` | Download subscription; returns summary plus redacted `profile` (serialized fetch lock) |
| `GET_PROFILE` | Cached profile with credentials redacted |
| `STATUS` | Daemon uptime, cache state, and sing-box backend state |
| `CONNECT` | Apply last fetched profile with `node_id`, generate sing-box config, start subprocess |
| `DISCONNECT` | Stop sing-box subprocess |

Place `sing-box.exe` in `engines/` next to `confluxd.exe`, or set `SINGBOX_PATH`.

Protocol reference: [docs/ipc-protocol.md](docs/ipc-protocol.md) · C# notes: [docs/windows-integration.md](docs/windows-integration.md)

## Configuration

Daemon config path: `CONFLUX_CONFIG` env var, else:

- Windows: `%APPDATA%\conflux.toml`
- Linux/macOS: `~/.config/conflux/conflux.toml`

```toml
# subscription_url = "https://example.com/sub/token"
pipe_name = "conflux-engine"
```

Example file: [conflux.toml.example](conflux.toml.example)

## Development

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release -p conflux-cli -p conflux-daemon
```

JSON Schema: [assets/schemas/normalized-profile.schema.json](assets/schemas/normalized-profile.schema.json)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Security

See [SECURITY.md](SECURITY.md). IPC responses redact credentials; use the Rust library directly when full secrets are required locally.

## License

Apache License 2.0 — see [LICENSE](LICENSE).
