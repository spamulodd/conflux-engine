# sing-box Backend

Conflux-engine uses [sing-box](https://sing-box.sagernet.org/) as the default data-plane backend. sing-box runs as a **managed subprocess**: conflux-engine generates configuration, spawns the binary, supervises its lifecycle, and reports status to IPC consumers.

## Why sing-box

| Criterion | sing-box | Alternative (mihomo / Clash Meta) |
|-----------|----------|-----------------------------------|
| Config model | Native JSON | Clash YAML-centric |
| Protocol coverage | Broad (VLESS, Hysteria2, TUIC, …) | Strong Clash-ecosystem focus |
| Rust integration | Subprocess (no stable Rust crate) | Same — Go binary |
| Config generation | Direct mapping from normalized nodes | Requires YAML translation layer |
| Process control | Clean start/stop via config swap | Similar, Clash-controller API |

Neither sing-box nor mihomo provides a Rust library suitable for embedding. Subprocess supervision offers crash isolation, simpler packaging, and independent version upgrades. The `Backend` trait in `conflux-backend` allows a future mihomo adapter without rewriting the core pipeline.

## Architecture

```
ConfluxSubscription
        │
        ▼
singbox::config::generate()
        │
        ▼
config.json (sing-box format)
        │
        ▼
singbox::process::spawn("sing-box run -c config.json")
        │
        ▼
TUN interface + outbound traffic
```

Conflux-engine never passes provider input directly to sing-box. All configuration is generated from schema-validated normalized profiles.

## Binary Discovery

sing-box is **not bundled** in conflux-engine v0.1.0. Provide the binary via one of:

| Method | Example |
|--------|---------|
| Config file | `[backend] binary = "C:\\Program Files\\sing-box\\sing-box.exe"` |
| Environment variable | `CONFLUX_SINGBOX_BIN=/usr/local/bin/sing-box` |
| PATH | `sing-box` on system PATH |

Future releases may offer optional bundled binaries in release artifacts.

### Version compatibility

| conflux-engine | sing-box | Notes |
|----------------|----------|-------|
| 0.1.x | ≥ 1.8.0 | VLESS Reality, Hysteria2, basic transports |
| 0.2.x (planned) | ≥ 1.9.0 | TUIC, expanded rule-set support |

Test against the sing-box version documented in release notes before deploying to production.

## Configuration Generation

### Input

A `ConfluxSubscription` with one selected `ConfluxNode` (or a default selector in v0.2).

### Output structure

```json
{
  "log": { "level": "info" },
  "dns": { "servers": [{ "tag": "dns-direct", "address": "1.1.1.1" }] },
  "inbounds": [
    {
      "type": "tun",
      "tag": "tun-in",
      "interface_name": "conflux-tun",
      "inet4_address": "172.19.0.1/30",
      "auto_route": true,
      "strict_route": true
    }
  ],
  "outbounds": [
    {
      "type": "vless",
      "tag": "proxy",
      "server": "203.0.113.10",
      "server_port": 443,
      "uuid": "00000000-0000-0000-0000-000000000000",
      "flow": "xtls-rprx-vision",
      "tls": {
        "enabled": true,
        "server_name": "example.com",
        "reality": {
          "enabled": true,
          "public_key": "…",
          "short_id": "…"
        }
      }
    },
    { "type": "direct", "tag": "direct" },
    { "type": "dns", "tag": "dns-out" }
  ],
  "route": {
    "rules": [
      { "protocol": "dns", "outbound": "dns-out" },
      { "inbound": "tun-in", "outbound": "proxy" }
    ],
    "final": "proxy"
  }
}
```

Values shown are illustrative; real configs are generated from normalized node fields.

### Protocol mapping

| ConfluxNode protocol | sing-box outbound `type` |
|---------------------|--------------------------|
| `vless` | `vless` |
| `vmess` | `vmess` |
| `shadowsocks` | `shadowsocks` |
| `trojan` | `trojan` |
| `hysteria2` | `hysteria2` |
| `tuic` | `tuic` |
| `wireguard` | `wireguard` |
| `socks` | `socks` |

Transport, TLS, and Reality fields map to sing-box `transport`, `tls`, and `tls.reality` blocks respectively.

## Process Lifecycle

```
Idle ──start()──► Starting ──process ready──► Running
  ▲                    │                          │
  │                    │ error                    │ stop()
  │                    ▼                          ▼
  └──── stop() ◄── Error ◄─────────────── Stopping
```

| State | Description |
|-------|-------------|
| `Idle` | No sing-box process |
| `Starting` | Config written, process spawned |
| `Running` | sing-box active, TUN up |
| `Stopping` | Graceful shutdown in progress |
| `Error` | Spawn failure, crash, or config rejection |

### Supervision

- stdout/stderr redirected to daemon log
- Crash detection via process exit code
- v0.1: full restart required on config change
- v0.2 (planned): hot reload via config swap where sing-box supports it

### Shutdown

1. Send termination signal to sing-box process
2. Wait up to 10 seconds for graceful exit
3. Force kill if still running
4. Remove temporary config file

## Known Limitations (v0.1)

- **XHTTP transport** — sing-box lacks native XHTTP; nodes using XHTTP are marked unsupported or mapped to `httpupgrade` with a warning
- **Full routing** — only basic full-tunnel routing; split tunneling and rule-set translation deferred to v0.2
- **Multi-node selection** — single outbound per connect; selector/urltest groups planned for v0.2
- **No auto-update** — sing-box binary must be installed separately

## Development Setup

### Windows

```powershell
# Download from https://github.com/SagerNet/sing-box/releases
# Extract sing-box.exe and add to PATH or config.toml
```

### Linux / macOS

```bash
# Example: install via package manager or download release binary
export CONFLUX_SINGBOX_BIN=/usr/local/bin/sing-box
```

### Integration tests

Integration tests that require sing-box are gated behind the `singbox-integration` feature flag and skipped in CI unless the binary is present:

```bash
cargo test --features singbox-integration -p conflux-backend
```

## Security Notes

- Generated config files are written to a restricted temporary directory and deleted on shutdown
- sing-box runs with the same privilege level as `confluxd` (typically elevated on Windows for TUN)
- Provider URIs never appear in sing-box logs; only generated config tags are logged

## References

- [sing-box configuration](https://sing-box.sagernet.org/configuration/)
- [Outbound types](https://sing-box.sagernet.org/configuration/outbound/)
- [TUN inbound](https://sing-box.sagernet.org/configuration/inbound/tun/)
