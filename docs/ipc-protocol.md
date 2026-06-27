# IPC Protocol

Conflux-engine exposes a **JSON-over-line** control protocol over a Windows named pipe. Desktop VPN clients send one request per connection; the daemon responds with a single line and closes the connection.

## Transport

| Property | Value |
|----------|-------|
| Platform | Windows (primary); Unix domain socket stub for CI |
| Pipe name | `\\.\pipe\conflux-engine` (configurable) |
| Encoding | UTF-8 |
| Framing | One JSON payload per line, terminated by `\n` (LF) |
| Connection model | One request → one response → disconnect |
| Protocol version | `1` |

### Pipe ACL

The daemon creates the pipe with read/write access for authenticated users so a standard-user desktop client can communicate with an elevated daemon host, mirroring common VPN service patterns.

## Message Format

### Request

```
{VERB} [{json_payload}]\n
```

- `VERB` — uppercase ASCII command name
- `json_payload` — optional JSON object; omitted for parameterless commands

### Response

Success:

```
OK {json_payload}\n
```

Failure:

```
ERR {message}\n
```

- `message` — human-readable error string (no nested JSON in v0.1)

Unknown commands return `ERR unknown command: {VERB}`.

## Protocol Version 1 Commands

### PING

Health check and version negotiation.

**Request:**

```
PING
```

**Response:**

```json
OK {"pong": true, "version": 1, "engine": "0.1.0"}
```

| Field | Type | Description |
|-------|------|-------------|
| `pong` | boolean | Always `true` on success |
| `version` | integer | IPC protocol version |
| `engine` | string | conflux-engine semver |

---

### FETCH

Download and parse a subscription URL. Updates the daemon's in-memory profile cache.

**Request:**

```
FETCH {"url": "https://example.com/sub/token"}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `url` | string | yes | HTTPS subscription URL |

**Response:**

```json
OK {
  "title": "Example VPN",
  "node_count": 42,
  "update_interval_hours": 12,
  "user_info": {
    "upload_bytes": 0,
    "download_bytes": 1073741824,
    "total_bytes": 0,
    "expire_unix": 1785099047
  }
}
```

On failure:

```
ERR fetch failed: HTTP 401 Unauthorized
```

The full normalized profile is retrieved via `GET_PROFILE`.

---

### GET_PROFILE

Return the current normalized subscription profile from cache.

**Request:**

```
GET_PROFILE
```

**Response:**

```json
OK {
  "title": "Example VPN",
  "source_url": "https://example.com/sub/token",
  "update_interval_hours": 12,
  "support_url": "https://example.com/support",
  "announce": "Welcome to Example VPN",
  "user_info": {
    "upload_bytes": 0,
    "download_bytes": 1073741824,
    "total_bytes": 0,
    "expire_unix": 1785099047,
    "refill_unix": null
  },
  "nodes": [
    {
      "id": "a1b2c3d4",
      "tag": "Example Node #1",
      "protocol": "vless",
      "server": "203.0.113.10",
      "port": 443,
      "meta": {
        "country_code": "FI",
        "flag": "🇫🇮"
      }
    }
  ]
}
```

Node objects in IPC responses omit sensitive credential fields. Full credentials are available to the backend internally; clients receive display-safe summaries.

If no profile is loaded:

```
ERR no profile loaded
```

---

### STATUS

Return daemon and backend runtime status.

**Request:**

```
STATUS
```

**Response:**

```json
OK {
  "daemon_state": "running",
  "backend_state": "idle",
  "profile_loaded": true,
  "profile_title": "Example VPN",
  "node_count": 42,
  "selected_node_id": null,
  "backend": {
    "type": "singbox",
    "state": "idle",
    "uptime_seconds": 0,
    "rx_bytes": 0,
    "tx_bytes": 0,
    "error": null
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `daemon_state` | string | `running`, `stopping`, `error` |
| `backend_state` | string | `idle`, `starting`, `running`, `stopping`, `error` |
| `profile_loaded` | boolean | Whether a profile is cached |
| `selected_node_id` | string \| null | Currently selected node (v0.2+) |
| `backend.rx_bytes` | integer | Received bytes (when running) |
| `backend.tx_bytes` | integer | Transmitted bytes (when running) |

## Planned Commands (v0.2+)

| Command | Purpose |
|---------|---------|
| `CONNECT` | Start backend with selected node and tunnel options |
| `DISCONNECT` | Stop backend and tear down tunnel |
| `SELECT_NODE` | Set active node without connecting |
| `IMPORT_SUBSCRIPTION` | Parse inline body or URL with format hint |
| `LIST_NODES` | Return node summaries (alias for profile subset) |
| `RELOAD` | Hot-reload backend configuration |
| `EVENTS` | Poll recent events (profile updated, state changed) |

## Wire Examples

Successful ping:

```
Client → PING\n
Server → OK {"pong":true,"version":1,"engine":"0.1.0"}\n
```

Fetch subscription:

```
Client → FETCH {"url":"https://example.com/sub/token"}\n
Server → OK {"title":"Example VPN","node_count":42,"update_interval_hours":12}\n
```

Invalid command:

```
Client → FOO\n
Server → ERR unknown command: FOO\n
```

## Client Implementation Notes

1. Open `NamedPipeClientStream` to `\\.\pipe\conflux-engine`
2. Write request bytes including trailing `\n`
3. Read until `\n` or connection close
4. Parse response prefix (`OK` or `ERR`)
5. Close the stream

Recommended polling interval for `STATUS`: 2 seconds while connected (matches common desktop client patterns).

### Timeouts

| Operation | Suggested timeout |
|-----------|-------------------|
| `PING` | 5 s |
| `FETCH` | 60 s (network-bound) |
| `GET_PROFILE` | 5 s |
| `STATUS` | 5 s |

## Schema Reference

The `GET_PROFILE` response body conforms to [normalized-profile.schema.json](../assets/schemas/normalized-profile.schema.json) with credential fields redacted for IPC transport.

## Compatibility

- Protocol version is negotiated via `PING`. Clients MUST check `version` and refuse incompatible versions.
- Additive JSON fields in responses are forward-compatible; clients MUST ignore unknown fields.
- Breaking changes increment the protocol version and use a new pipe name suffix if needed (e.g. `conflux-engine.v2`).
