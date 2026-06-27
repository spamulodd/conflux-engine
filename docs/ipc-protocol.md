# IPC Protocol

Conflux-engine exposes a **JSON-over-line** control protocol over a Windows named pipe (Unix domain socket on Linux/macOS for CI). Each connection carries one request line and one response line.

## Transport

| Property | Value |
|----------|-------|
| Platform | Windows named pipe; Unix socket at `/tmp/conflux-engine.sock` on non-Windows |
| Pipe name | `\\.\pipe\conflux-engine` (configurable via `pipe_name` in daemon config) |
| Encoding | UTF-8 |
| Framing | One JSON object per line, terminated by `\n` (LF) |
| Connection model | Client may send multiple requests on one connection (line-delimited) |
| Protocol version | `1` (`v` field in every message) |

## Message Format

### Request envelope

```json
{"v":1,"cmd":"PING"}
{"v":1,"cmd":"FETCH","url":"https://example.com/sub/token"}
{"v":1,"cmd":"GET_PROFILE"}
{"v":1,"cmd":"STATUS"}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `v` | integer | yes | Protocol version (`1`) |
| `cmd` | string | yes | `PING`, `FETCH`, `GET_PROFILE`, or `STATUS` |
| `url` | string | for `FETCH` | HTTPS subscription URL |

### Response envelope

Success:

```json
{"v":1,"status":"OK","data":{...}}
```

Failure:

```json
{"v":1,"status":"ERR","msg":"human-readable error"}
```

| Field | Type | Description |
|-------|------|-------------|
| `v` | integer | Protocol version |
| `status` | string | `OK` or `ERR` |
| `data` | object | Present on success |
| `msg` | string | Present on failure |

## Protocol Version 1 Commands

### PING

Health check and version negotiation.

**Request:** `{"v":1,"cmd":"PING"}`

**Response `data`:**

```json
{
  "pong": true,
  "version": 1,
  "engine": "0.1.0"
}
```

---

### FETCH

Download and parse a subscription URL. Updates the daemon in-memory profile cache.

**Request:** `{"v":1,"cmd":"FETCH","url":"https://example.com/sub/token"}`

**Response `data` (summary only — no nodes or credentials):**

```json
{
  "title": "Example VPN",
  "node_count": 42,
  "update_interval_hours": 12,
  "user_info": {
    "upload_bytes": 0,
    "download_bytes": 1073741824,
    "total_bytes": 0,
    "expire_unix": 1785099047
  },
  "support_url": "https://example.com/support",
  "announce": "Welcome"
}
```

On failure: `{"v":1,"status":"ERR","msg":"fetch failed: HTTP 401 Unauthorized"}`

Use `GET_PROFILE` after a successful fetch to retrieve the cached profile (credentials redacted).

---

### GET_PROFILE

Return the cached normalized profile.

**Request:** `{"v":1,"cmd":"GET_PROFILE"}`

**Response `data`:** normalized profile JSON with `credentials`, raw URIs, and private keys replaced by `"[redacted]"`.

If no profile is loaded: `{"v":1,"status":"ERR","msg":"no profile loaded"}`

---

### STATUS

Return daemon runtime status (v0.1 — no sing-box backend state yet).

**Request:** `{"v":1,"cmd":"STATUS"}`

**Response `data`:**

```json
{
  "version": "0.1.0",
  "protocol_version": 1,
  "uptime_secs": 3600,
  "has_profile": true,
  "node_count": 42,
  "title": "Example VPN",
  "last_fetch_url": "https://example.com/sub/token",
  "last_error": null
}
```

Backend lifecycle fields (`backend_state`, `rx_bytes`, `tx_bytes`) arrive in v0.2 with `CONNECT` / `DISCONNECT`.

## Planned Commands (v0.2+)

| Command | Purpose |
|---------|---------|
| `CONNECT` | Start sing-box backend with selected node |
| `DISCONNECT` | Stop backend and tear down tunnel |
| `SELECT_NODE` | Set active node without connecting |
| `RELOAD` | Hot-reload backend configuration |
| `EVENTS` | Poll daemon/backend events |

## Wire Examples

Successful ping:

```
Client → {"v":1,"cmd":"PING"}\n
Server → {"v":1,"status":"OK","data":{"pong":true,"version":1,"engine":"0.1.0"}}\n
```

Fetch subscription:

```
Client → {"v":1,"cmd":"FETCH","url":"https://example.com/sub/token"}\n
Server → {"v":1,"status":"OK","data":{"title":"Example VPN","node_count":42,"update_interval_hours":12}}\n
```

Invalid JSON:

```
Client → not json\n
Server → {"v":1,"status":"ERR","msg":"invalid request JSON: ..."}\n
```

## Client Implementation Notes

1. Open `NamedPipeClientStream` to `\\.\pipe\conflux-engine` (Windows) or connect to the Unix socket path.
2. Write one JSON request per line (include trailing `\n`).
3. Read one line JSON response.
4. Check `status` (`OK` / `ERR`).

See [windows-integration.md](windows-integration.md) for a C# example.

### Timeouts

| Operation | Suggested timeout |
|-----------|-------------------|
| `PING` | 5 s |
| `FETCH` | 60 s |
| `GET_PROFILE` | 5 s |
| `STATUS` | 5 s |

## Compatibility

- Clients MUST send `v: 1` and reject unsupported versions.
- Unknown JSON fields in `data` are forward-compatible; ignore them.
- Breaking changes increment the protocol version.
