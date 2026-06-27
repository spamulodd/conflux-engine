# Subscription Formats

This document catalogs subscription formats supported (or planned) by conflux-engine, with detection heuristics and parsing notes.

## Detection Pipeline

Apply these steps in order after HTTP fetch or on local file input:

```
1. Parse HTTP headers (Subscription-Userinfo, Profile-Title, etc.)
2. Strip BOM and leading/trailing whitespace from body
3. If body starts with '<' → reject as HTML error page
4. If no scheme:// prefix and no YAML/JSON markers → Base64 decode
5. Split on \r?\n; strip # directive lines
6. Classify expanded content (see branches below)
7. Attach subscription metadata from headers and body directives
```

## HTTP Envelope

All formats arrive via HTTP(S). Common response signals:

| Signal | Heuristic |
|--------|-----------|
| Status `401` | Invalid or expired token |
| `Content-Type: text/plain` | URI list or Base64 body |
| `Content-Type: application/yaml` | Clash YAML |
| `Content-Type: application/json` | sing-box / Xray JSON |
| `Subscription-Userinfo` | Clash-ecosystem quota header |
| `Profile-Title` | Plain text or `base64:…` encoded title |
| `Profile-Update-Interval` | Refresh interval in hours (or seconds if ≥ 3600) |
| `Announce` | `base64:…` provider banner |
| `Support-Url` | Provider support link |
| `Subscription-Refill-Date` | Unix timestamp for traffic reset |
| `Content-Disposition` | Fallback title from `filename=` |
| `Flclashx-Background` | FLClashX wallpaper URL (metadata only) |

Some panels return different body formats based on User-Agent (`Clash`, `ClashMeta`, `sing-box`, `v2rayN`). Conflux-engine uses a configurable User-Agent and may retry with alternate values.

## Format Reference

### Base64-encoded URI list

**Most common commercial format (~80%+ of subscriptions).**

| Property | Value |
|----------|-------|
| Detection | Body matches `[A-Za-z0-9+/=_-]+`; decoded lines contain `scheme://` |
| Structure | `base64(node1\nnode2\n…\nnodeN)` |
| Typical headers | `Subscription-Userinfo`, `Profile-Title`, `Profile-Update-Interval` |
| v0.1.0 | Supported |

### Plaintext URI list

| Property | Value |
|----------|-------|
| Detection | Lines contain scheme prefixes without Base64 wrapper |
| Directives | `#profile-title:`, `#subscription-userinfo:`, `#profile-update-interval:` |
| v0.1.0 | Supported |

### URI Schemes

| Scheme | Structure | v0.1.0 |
|--------|-----------|--------|
| `vless://` | `UUID@host:port?query#label` — Reality, XTLS flow, transports | Supported |
| `vmess://` | Base64 JSON (`add`, `port`, `id`, `net`, `tls`, …) | Supported |
| `ss://` | SIP002 `ss://BASE64(method:pass)@host:port#tag` or legacy | Supported |
| `trojan://` | `password@host:port?security=tls&sni=…#label` | Supported |
| `hysteria2://` | `password@host:port/?sni=…#label` | Supported |
| `hy2://` | Alias for `hysteria2://` (normalized on import) | Supported |
| `hysteria://` | Hysteria v1 (legacy) | Planned |
| `tuic://` | TUIC v4/v5 | Planned |
| `wireguard://` | Base64 or query params | Planned |
| `ssr://` | Legacy ShadowsocksR | Planned |
| `socks://` / `socks5://` | SOCKS proxy URI | Planned |

#### VLESS query parameters

`security`, `type`, `flow`, `sni`, `fp`, `pbk`, `sid`, `host`, `path`, `serviceName`, `headerType`, `encryption`, `alpn`, `packetEncoding`

#### VMess variants

- **v2:** JSON contains `"v": "2"`
- **v1:** Flat JSON without version field

#### Shadowsocks branches

Decode segment before `@`:
- If `method:password` → SIP002
- Else decode whole string as legacy format

### Clash YAML / Clash Meta / Mihomo

| Property | Value |
|----------|-------|
| Detection | Top-level `proxies:`, `proxy-groups:`, or `rules:` keys; or `Content-Type: application/yaml` |
| Node location | `proxies[]` array |
| Node fields | `name`, `type`, `server`, `port`, plus type-specific options |
| Extensions | Reality (`reality-opts`), `client-fingerprint`, `grpc-opts`, `ws-opts`, `hysteria2` type |
| Not extracted | `proxy-groups`, `rules`, `dns`, `tun` (routing metadata) |
| v0.1.0 | Partial — `proxies[]` extraction only |

Subconverter URLs with `?flag=clash` or `?target=clash` produce this format.

### sing-box JSON

| Property | Value |
|----------|-------|
| Detection | JSON object with `"outbounds": [...]` array |
| Alternate | JSON array of outbound objects with `"type"` field |
| Node types | `vless`, `vmess`, `trojan`, `shadowsocks`, `hysteria2`, `tuic`, `wireguard`, `socks` |
| Skipped | `selector`, `urltest`, `direct`, `block` outbounds |
| v0.1.0 | Partial — protocol outbounds only |

sing-box has no native subscription URL format; providers serve URI lists or pre-built JSON.

### Xray JSON

| Property | Value |
|----------|-------|
| Detection | JSON with `"protocol": "vless"` (Xray schema, not sing-box `"type"`) |
| Transform | Map `protocol` → internal type; nested `settings`, `streamSettings` |
| v0.1.0 | Planned |

### Happ-specific extensions

Happ consumes the same URI list formats with additional metadata:

**HTTP headers:** `routing-enable`, `routing` (base64 JSON routing profile)

**Body directives:**

```
#profile-title: Example VPN
#subscription-userinfo: upload=0; download=1073741824; total=0; expire=1785099047
#profile-update-interval: 12
```

Conflux-engine extracts directive metadata and overlays it on the normalized subscription. Happ-only routing profiles are passthrough in v0.1.

### FLClashX

Not a separate body format. FLClashX consumes Clash Meta YAML or Base64 URI lists. The `Flclashx-Background` header is stored as subscription metadata only.

### Generic HTTP subscription endpoint

Pattern: `GET https://host/path/{token}` → raw body

| Panel style | Typical body | Headers |
|-------------|-------------|---------|
| V2Board / Xboard / Marzban | Base64 URI list | Full Clash-style headers |
| Remnawave | Base64 URI list | `Announce`, `Subscription-Refill-Date` |
| 3x-ui | Base64 or Clash (by UA) | Optional routing headers |
| subconverter | Depends on `target`/`flag` query | Pass-through |

Query-param format selection: `?flag=clash`, `?target=sing-box`, `?target=v2ray` — conflux-engine may append these on retry.

## Parser Priority

Implementation order by frequency and integration value:

| Priority | Format |
|----------|--------|
| P0 | HTTP fetch + header parser |
| P0 | Base64 expand + plaintext URI list |
| P1 | `vless://`, `ss://`, `trojan://`, `hysteria2://` |
| P2 | `vmess://` |
| P3 | Clash YAML `proxies[]`, sing-box JSON outbounds |
| P4 | Happ body directives, `tuic://`, `wireguard://` |
| P5 | Xray JSON, legacy schemes (`hysteria://`, `ssr://`, `socks://`) |

## Normalized Output

All parsers emit `ConfluxSubscription` containing `ConfluxNode` entries. See [normalized-profile.schema.json](../assets/schemas/normalized-profile.schema.json).

Each node preserves a `raw` reference to the original URI line or YAML/JSON subtree for round-trip debugging.

## Test Vectors

Development fixtures should cover:

1. Base64 mixed list (vless + ss + trojan)
2. Plaintext URI list with `#profile-title` directives
3. Clash YAML from 3x-ui
4. sing-box JSON from subconverter
5. VMess v1 and v2 URIs
6. Hysteria2 with `hy2://` alias
7. Invalid/expired: HTTP 401, placeholder nodes

**Never commit real subscription URLs with live tokens.** Use `https://example.com/sub/token` in documentation and tests.

## User-Agent Fallback Strategy

When auto-detection fails:

1. Retry with `User-Agent: ClashMeta`
2. Retry with `User-Agent: sing-box`
3. Retry with `?flag=clash` appended to URL
4. Report `ParseFailed` with diagnostic context
