# conflux-engine

Universal subscription fetch, parse, and normalize engine for proxy and VPN clients. Conflux-engine downloads provider subscriptions, detects their format, maps nodes to a canonical profile model, and can drive a [sing-box](https://sing-box.sagernet.org/) backend. It ships as a CLI, a long-running daemon, and a Windows IPC service for integration with desktop VPN clients.

## Features

- **HTTP subscription fetch** — TLS via rustls, standard subscription headers (`Profile-Title`, `Subscription-Userinfo`, `Profile-Update-Interval`, `Announce`, `Support-Url`), and conditional caching
- **Multi-format parsing** — Base64 URI lists, plaintext URI lines, Clash YAML (`proxies[]`), and sing-box JSON outbounds
- **Protocol coverage** — VLESS, VMess, Shadowsocks, Trojan, Hysteria2, TUIC, WireGuard, SOCKS, and provider-specific URI schemes
- **Normalized profile model** — backend-agnostic `ConfluxSubscription` / `ConfluxNode` types with a published JSON Schema
- **sing-box backend** — translate normalized profiles to sing-box config and supervise the data-plane process
- **Windows IPC** — JSON line protocol over a named pipe for desktop VPN client integration
- **CLI tools** — `conflux` for fetch/convert/validate; `confluxd` for daemon mode

## Quick Start

### Prerequisites

- Rust **1.80+** (see `rust-toolchain.toml`)
- Optional: [sing-box](https://sing-box.sagernet.org/) binary for tunnel mode (see [docs/backend-singbox.md](docs/backend-singbox.md))

### Install from source

```bash
git clone https://github.com/spamulodd/conflux-engine.git
cd conflux-engine
cargo build --release
```

Binaries are produced at `target/release/conflux` and `target/release/confluxd`.

### CLI examples

Fetch a subscription and write a normalized profile:

```bash
conflux fetch https://example.com/sub/token --output profile.json
```

Convert a local subscription file:

```bash
conflux convert subscription.txt --format auto --output profile.json
```

Validate a normalized profile against the JSON Schema:

```bash
conflux validate profile.json
```

Run the IPC daemon (Windows):

```bash
confluxd
# listens on \\.\pipe\conflux-engine
```

### Library usage

```rust
use conflux_engine::fetch_and_normalize;

let subscription = fetch_and_normalize("https://example.com/sub/token").await?;
```

See [examples/basic-fetch/src/main.rs](examples/basic-fetch/src/main.rs) for a complete example.

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
| `conflux-ipc` | Windows named-pipe IPC server and client |
| `conflux-cli` | `conflux` binary |
| `conflux-daemon` | `confluxd` binary |
| `conflux-engine` | Root library facade re-exporting core + protocol |

Detailed design: [docs/architecture.md](docs/architecture.md)

## Supported Subscription Formats

| Format | Detection | v0.1.0 status |
|--------|-----------|---------------|
| Base64-encoded URI list | Body is Base64; decoded lines contain `scheme://` | Supported |
| Plaintext URI list | Lines with `vless://`, `ss://`, etc. | Supported |
| VLESS URI | `vless://` prefix | Supported |
| VMess URI | `vmess://` + Base64 JSON | Supported |
| Shadowsocks URI | `ss://` (SIP002 and legacy) | Supported |
| Trojan URI | `trojan://` | Supported |
| Hysteria2 URI | `hysteria2://` or `hy2://` | Supported |
| Clash / Mihomo YAML | Top-level `proxies:` key | Partial (`proxies[]` only) |
| sing-box JSON | `"outbounds"` array | Partial (protocol outbounds) |
| TUIC / WireGuard URI | `tuic://`, `wireguard://` | Planned |
| Happ body directives | `#profile-title:` comment lines | Metadata overlay |
| Xray JSON array | `"protocol"` field | Planned |

Full detection heuristics: [docs/subscription-formats.md](docs/subscription-formats.md)

## sing-box Backend

Conflux-engine does not embed a proxy core. It generates sing-box configuration from normalized profiles and supervises a sing-box subprocess for the data plane. This keeps subscription intelligence separate from traffic handling and allows independent sing-box upgrades.

See [docs/backend-singbox.md](docs/backend-singbox.md) for configuration mapping, binary discovery, and version compatibility.

## Windows IPC Integration

Desktop VPN clients integrate via a JSON-over-line protocol on the named pipe `\\.\pipe\conflux-engine`. The client sends one request per connection; the daemon responds with `OK {json}` or `ERR {message}`.

Initial commands: `PING`, `FETCH`, `GET_PROFILE`, `STATUS`.

See [docs/ipc-protocol.md](docs/ipc-protocol.md) and [docs/windows-integration.md](docs/windows-integration.md).

## Configuration

Daemon configuration is loaded from the path in the `CONFLUX_CONFIG` environment variable, or a default location:

```toml
# config.toml
[subscription]
url = "https://example.com/sub/token"
refresh_interval_hours = 12

[backend]
type = "singbox"
binary = "C:\\Program Files\\sing-box\\sing-box.exe"

[ipc]
pipe_name = "conflux-engine"

[logging]
level = "info"
```

## Development

```bash
# Format check
cargo fmt --all -- --check

# Lint
cargo clippy --workspace --all-targets -- -D warnings

# Test
cargo test --workspace

# Build release binaries
cargo build --release -p conflux-cli -p conflux-daemon
```

Workspace layout and module responsibilities are documented in [docs/architecture.md](docs/architecture.md).

### JSON Schema

The normalized profile schema lives at [assets/schemas/normalized-profile.schema.json](assets/schemas/normalized-profile.schema.json).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Security

See [SECURITY.md](SECURITY.md) for vulnerability reporting.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE).
