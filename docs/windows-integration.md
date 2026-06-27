# Windows Integration

This document describes how desktop VPN clients integrate with conflux-engine on Windows: process hosting, named pipe IPC, privilege model, and recommended deployment layout.

## Integration Model

Desktop VPN clients remain responsible for UI, settings persistence, and user-facing connection state. Conflux-engine handles subscription parsing, backend configuration, and proxy/tunnel execution.

```
┌──────────────────────────────────────────────────────────────┐
│                    Desktop VPN Client                        │
│  ┌─────────────┐  ┌──────────────────┐  ┌───────────────┐  │
│  │ UI / Tray   │  │ Profile store    │  │ Connect flow  │  │
│  └──────┬──────┘  └────────┬─────────┘  └───────┬───────┘  │
│         │                  │                     │          │
│         └──────────────────┼─────────────────────┘          │
│                            │                                 │
│                   ┌────────▼────────┐                        │
│                   │ ConfluxEngine   │                        │
│                   │ Client (C#/etc) │                        │
│                   └────────┬────────┘                        │
└────────────────────────────┼─────────────────────────────────┘
                             │ NamedPipeClientStream
                             │ \\.\pipe\conflux-engine
┌────────────────────────────▼─────────────────────────────────┐
│                    confluxd (daemon)                          │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │ IPC server  │  │ conflux-core │  │ conflux-backend  │  │
│  └─────────────┘  └──────────────┘  └────────┬─────────┘  │
└──────────────────────────────────────────────┼──────────────┘
                                               │
                                      ┌────────▼────────┐
                                      │ sing-box.exe      │
                                      │ (TUN + outbound)  │
                                      └───────────────────┘
```

## Named Pipe

| Property | Value |
|----------|-------|
| Default name | `\\.\pipe\conflux-engine` |
| Protocol | JSON-over-line (see [ipc-protocol.md](ipc-protocol.md)) |
| Connection | One request per connection |
| Encoding | UTF-8 |

### Client example (C#)

```csharp
using System.IO.Pipes;
using System.Text;

async Task<string> SendCommandAsync(string request)
{
    using var pipe = new NamedPipeClientStream(
        ".", "conflux-engine", PipeDirection.InOut);
    await pipe.ConnectAsync(5000);

    var requestBytes = Encoding.UTF8.GetBytes(request + "\n");
    await pipe.WriteAsync(requestBytes);

    using var reader = new StreamReader(pipe, Encoding.UTF8);
    return await reader.ReadLineAsync()
        ?? throw new IOException("Empty response from conflux-engine");
}

// Usage:
var response = await SendCommandAsync("PING");
// → OK {"pong":true,"version":1,"engine":"0.1.0"}

var response = await SendCommandAsync(
    "FETCH {\"url\":\"https://example.com/sub/token\"}");
```

## Process Management

### Recommended: elevated host

TUN adapter creation and route modification require administrator privileges on Windows. Recommended deployment:

1. Install `confluxd.exe` alongside the VPN client application
2. A privileged host process (Windows service or elevated helper) starts and supervises `confluxd`
3. The standard-user UI communicates via the named pipe

### Install layout

```
{app}/
  Client.exe
  engines/
    conflux.exe          # CLI (optional, for diagnostics)
    confluxd.exe         # Daemon
  config/
    conflux.toml         # Default daemon config
```

Environment variable `CONFLUX_CONFIG` overrides the config file path.

### Spawn sketch (C#)

```csharp
var exe = Path.Combine(installRoot, "engines", "confluxd.exe");
var psi = new ProcessStartInfo
{
    FileName = exe,
    WorkingDirectory = Path.GetDirectoryName(exe)!,
    CreateNoWindow = true,
    UseShellExecute = false,
    RedirectStandardError = true,
};
// Optional: CONFLUX_CONFIG, CONFLUX_SINGBOX_BIN
var process = Process.Start(psi);
// Watchdog: restart on unexpected exit
```

### Lifecycle events

| Event | Client action |
|-------|---------------|
| App start | Ensure daemon running (`PING`) |
| Import subscription | `FETCH` then `GET_PROFILE` |
| Connect (v0.2+) | `CONNECT` with node and tunnel options |
| Disconnect (v0.2+) | `DISCONNECT` |
| Status polling | `STATUS` every 2 s while active |
| App exit | Optional: leave daemon running for fast reconnect |

## Privilege and Security Model

| Component | Privilege | Rationale |
|-----------|-----------|-----------|
| Desktop client UI | Standard user | No admin required for daily use |
| confluxd | Elevated (or SYSTEM service) | TUN creation, route table |
| sing-box subprocess | Same as confluxd | Inherits TUN handle |
| Named pipe | Authenticated users RW | UI can reach elevated daemon |

Pipe ACL should allow authenticated users read/write access, matching common VPN service patterns where a SYSTEM-level service accepts commands from the standard-user application.

## Responsibility Split

| Task | Desktop client | conflux-engine |
|------|----------------|----------------|
| UI, settings, tray | ✓ | |
| Profile/node persistence | ✓ | Returns normalized DTOs |
| External subscription parse | Orchestrate via IPC | ✓ |
| Native protocol tunnel | Client's own engine | |
| External protocol tunnel | Orchestrate via IPC | ✓ (via sing-box) |
| TUN creation (external) | | ✓ |
| Route/DNS configuration | Optional coordination | ✓ (via sing-box TUN inbound) |
| Kill switch / app routing | ✓ (WFP rules) | Coordinate on TUN interface name |
| Status display | Poll + render | Emit via `STATUS` |
| Process lifecycle | Spawn, monitor, restart | Run until stopped |

## Profile Mapping

When importing via `GET_PROFILE`, map each node to the client's profile store:

| Conflux field | Client field (example) |
|---------------|------------------------|
| `id` | Stable node key |
| `tag` | Display name |
| `protocol` | Protocol enum |
| `server` + `port` | Endpoint |
| `meta.country_code` | Country filter |
| `meta.flag` | UI emoji |

Store an opaque `config_ref` or the full normalized node JSON for connect-time use. Mark profiles as external protocol (not the client's native wire protocol).

## TUN Interface Coordination

sing-box creates a TUN adapter (default name: `conflux-tun`). The desktop client should:

1. Query `STATUS` for the active interface name
2. Avoid conflicting adapters (other VPN clients, Tailscale, etc.)
3. Coordinate kill-switch rules to the correct interface index

## Logging

| Component | Recommended location |
|-----------|---------------------|
| confluxd | `%ProgramData%\Conflux\logs\confluxd.log` |
| sing-box stderr | Same directory, rotated by daemon |
| Client IPC trace | Client's existing log directory (debug builds) |

Set log level via config:

```toml
[logging]
level = "info"   # trace, debug, info, warn, error
```

## v0.1 Integration Checklist

- [ ] Bundle `confluxd.exe` in installer under `engines/`
- [ ] Implement `ConfluxEngineClient` with pipe send/receive
- [ ] Start daemon from elevated host on first external-protocol use
- [ ] `PING` health check on app start
- [ ] `FETCH` + `GET_PROFILE` for subscription import
- [ ] Map nodes to client profile store
- [ ] `STATUS` polling loop (2 s interval)
- [ ] Handle daemon crash (restart + user notification)

## v0.2 Additions

- `CONNECT` / `DISCONNECT` for full tunnel lifecycle
- `SELECT_NODE` for persistent node selection
- Windows Service installation (`confluxd install`)
- Event stream for profile refresh notifications
- Kill-switch integration with TUN interface name

## Troubleshooting

| Symptom | Likely cause | Action |
|---------|-------------|--------|
| `ConnectAsync` timeout | Daemon not running | Start `confluxd` from elevated host |
| `ERR no profile loaded` | Missing `FETCH` | Call `FETCH` before `GET_PROFILE` |
| `ERR fetch failed: HTTP 401` | Expired subscription token | Prompt user to renew |
| Backend state `error` | sing-box not found | Set `CONFLUX_SINGBOX_BIN` or install sing-box |
| TUN creation failure | Insufficient privileges | Run daemon elevated |

## Related Documents

- [IPC Protocol](ipc-protocol.md) — message reference
- [Architecture](architecture.md) — crate layout and data flow
- [sing-box Backend](backend-singbox.md) — backend configuration
