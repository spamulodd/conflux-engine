# Architecture

This document describes the internal architecture of conflux-engine: crate boundaries, data flow, and design decisions.

## Goals

1. **Backend-agnostic subscription intelligence** — fetch, detect, parse, and normalize provider subscriptions into a canonical model before any proxy core sees them.
2. **Clean integration surface** — desktop VPN clients talk to a stable IPC protocol; they do not embed parser logic.
3. **Pluggable data plane** — sing-box is the default backend; the `Backend` trait allows future adapters without rewriting the core pipeline.
4. **Cross-platform library** — core crates compile on Windows, Linux, and macOS; IPC is Windows-first with Unix socket stubs for CI.

## Workspace Layout

```
conflux-engine/          # Root facade library (re-exports core + protocol)
crates/
  conflux-protocol/      # Domain types, errors, schema types
  conflux-core/          # fetch → parse → normalize pipeline
  conflux-backend/       # Backend trait + sing-box adapter
  conflux-ipc/           # Named-pipe IPC (Windows)
  conflux-cli/           # `conflux` binary
  conflux-daemon/        # `confluxd` binary
```

### Dependency graph

```
conflux-protocol
       ↑
conflux-core
       ↑
conflux-backend
       ↑
conflux-ipc ──→ conflux-daemon
conflux-cli (depends on core; optional ipc)
```

Libraries are layered so that `conflux-core` has no dependency on IPC or backend code. Consumers who only need parsing can depend on `conflux-core` or the root `conflux-engine` facade.

## Data Flow

### Subscription import pipeline

```
HTTP URL or local file
        │
        ▼
   ┌─────────┐
   │  fetch  │  reqwest + rustls; parse subscription headers
   └────┬────┘
        │ raw body + SubscriptionHeaders
        ▼
   ┌─────────┐
   │  parse  │  detect format; expand Base64; extract nodes
   └────┬────┘
        │ ParsedSubscription (format-specific intermediate)
        ▼
   ┌───────────┐
   │ normalize │  map to ConfluxNode / ConfluxSubscription
   └─────┬─────┘
         │ ConfluxSubscription (JSON-serializable)
         ▼
   ┌──────────────┐        ┌─────────────┐
   │ IPC / CLI    │   or   │ conflux-    │
   │ consumers    │        │ backend     │
   └──────────────┘        └──────┬──────┘
                                  │ sing-box JSON config
                                  ▼
                           sing-box subprocess
```

### Runtime connect flow (daemon + backend)

```
Desktop VPN client
        │ CONNECT (future) / FETCH + GET_PROFILE (v0.1)
        ▼
   conflux-ipc server
        │
        ├──► conflux-core (refresh subscription if stale)
        │
        └──► conflux-backend
                  │
                  ├── generate sing-box config
                  ├── spawn / supervise sing-box
                  └── report STATUS (state, bytes, errors)
```

## Crate Responsibilities

### conflux-protocol

Stable domain types shared by all crates:

| Type | Purpose |
|------|---------|
| `ConfluxSubscription` | Title, metadata, node list, source URL |
| `ConfluxNode` | Normalized node: protocol, endpoint, credentials, transport, TLS |
| `Protocol` | Enum of supported proxy protocols |
| `SubscriptionHeaders` | Parsed HTTP subscription headers |
| `SubscriptionUserInfo` | Upload/download/total/expire quota |
| `ConfluxError` | Stable error taxonomy for IPC and CLI |

These types are the contract between parsers, IPC, CLI, and backend adapters. The JSON Schema in `assets/schemas/normalized-profile.schema.json` mirrors `ConfluxSubscription`.

### conflux-core

| Module | Responsibility |
|--------|----------------|
| `fetch` | HTTP(S) download, header extraction, User-Agent, timeouts |
| `parse` | Format detection, Base64 expansion, URI line extraction |
| `parse::clash` | Clash YAML `proxies[]` extraction |
| `normalize` | Map parsed nodes → `ConfluxNode` with stable IDs |

Public API:

- `fetch_subscription(url)` — fetch and return `ConfluxSubscription`
- `parse_body(body, headers)` — parse a local body with optional headers
- `normalize(parsed)` — normalize intermediate parse result

### conflux-backend

| Module | Responsibility |
|--------|----------------|
| `traits::Backend` | `apply`, `start`, `stop`, `reload`, `health` |
| `singbox::config` | `ConfluxSubscription` → sing-box JSON |
| `singbox::process` | Locate binary, spawn, supervise, graceful shutdown |
| `runtime` | State machine: Idle → Starting → Running → Error |

Conflux-engine owns subscription intelligence; sing-box owns the data plane. The backend crate never parses subscriptions — it only consumes normalized profiles.

### conflux-ipc

| Module | Responsibility |
|--------|----------------|
| `protocol` | Message framing, version negotiation, encode/decode |
| `server` | Named pipe listener (`\\.\pipe\conflux-engine`) |
| `client` | Client SDK for desktop VPN clients and tests |

One request per pipe connection: client writes a line, server writes a line, connection closes.

### conflux-cli / conflux-daemon

- **`conflux`** — developer and operator tool: fetch, convert, validate, foreground daemon
- **`confluxd`** — long-running service: IPC listener, optional subscription refresh scheduler, backend lifecycle

## Format Detection Strategy

Detection runs after HTTP fetch (or on local input):

1. Strip BOM and whitespace
2. Reject HTML error pages (`<` prefix)
3. Attempt Base64 decode if no known scheme prefix is present
4. Strip `#`-directive comment lines (Happ-style body metadata)
5. Branch:
   - YAML with `proxies:` → Clash parser
   - JSON with `outbounds` → sing-box parser
   - Otherwise → URI line scanner

See [subscription-formats.md](subscription-formats.md) for heuristics per format.

## Node Identity

Each `ConfluxNode` receives a stable `id` derived from a normalized connection fingerprint (protocol, server, port, credentials hash, transport). This ID survives subscription refreshes when the underlying endpoint is unchanged, allowing desktop clients to preserve user selection across updates.

## Error Handling

Errors propagate as `ConfluxError` with stable codes:

| Code | Meaning |
|------|---------|
| `FetchFailed` | HTTP or network error |
| `ParseFailed` | Unrecognized or malformed body |
| `NormalizeFailed` | Known format but unsupported fields |
| `ValidationFailed` | Schema validation error |
| `BackendError` | sing-box spawn or config error |
| `IpcError` | Protocol or pipe error |

IPC surfaces errors as `ERR {message}`; CLI prints to stderr and exits non-zero.

## Threading and Async Model

- **conflux-core** fetch uses async reqwest (Tokio)
- **conflux-ipc** server runs on Tokio; one connection handled at a time in v0.1
- **conflux-backend** process supervision uses Tokio `process` + signal handling

## Security Boundaries

| Component | Trust level |
|-----------|-------------|
| Subscription URL | Untrusted input; fetched over HTTPS only |
| Parsed URIs | Untrusted; validated before normalization |
| Normalized profile | Internal; schema-validated before backend apply |
| sing-box config | Generated locally; never executed from provider input directly |
| IPC pipe | Local machine only; ACL restricts to authenticated users |

## Versioning

- **Library semver** — follows Cargo workspace version
- **IPC protocol version** — integer in `PING` response; breaking changes increment version
- **JSON Schema** — `$id` includes major version; additive fields are backward compatible

## Future Extensions (post v0.1)

- Windows Service installation for `confluxd`
- IPC commands: `CONNECT`, `DISCONNECT`, `SELECT_NODE`, event stream
- Subscription refresh scheduler with diff notifications
- Optional mihomo backend adapter behind feature flag
- Full Clash routing rule translation to sing-box rule-sets
