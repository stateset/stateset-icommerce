# Permissions & Auth

The HTTP gateway supports API key authentication, per-route permission levels,
and a sandbox mode that blocks dangerous routes. All three are disabled by
default for backwards compatibility.

## Quick Start

Add an `apiKeys` array to your gateway config to enable authentication:

```json
{
  "httpGateway": {
    "enabled": true,
    "port": 8080,
    "host": "127.0.0.1",
    "apiKeys": [
      { "key": "sk-prod-abc123", "name": "admin-key", "level": "admin" },
      { "key": "sk-read-xyz789", "name": "dashboard",  "level": "read" }
    ]
  }
}
```

Once at least one key is configured, every request (except `GET /health`) must
include a valid token.

## Permission Levels

Levels are numeric and hierarchical — a higher level grants access to
everything below it.

| Level     | Value | Description                                   |
|-----------|-------|-----------------------------------------------|
| `none`    | 0     | Public routes only (`/health`)                |
| `read`    | 1     | Read-only endpoints (metrics, status, lists)  |
| `preview` | 2     | Reserved for future preview features          |
| `write`   | 3     | Create and update operations                  |
| `delete`  | 4     | Destructive operations (memory deletion, etc.)|
| `admin`   | 5     | Full access including daemon and remote-access|

## Route Permission Map

Each route prefix has a base permission level. Some routes override the level
for specific HTTP methods.

| Route Prefix      | Base Level | Method Overrides                    |
|--------------------|-----------|-------------------------------------|
| `/health`          | `none`    |                                     |
| `/metrics`         | `read`    |                                     |
| `/commands`        | `read`    |                                     |
| `/skills`          | `read`    |                                     |
| `/plugins`         | `read`    | `POST` requires `admin`             |
| `/daemon`          | `admin`   |                                     |
| `/remote-access`   | `admin`   |                                     |
| `/voice`           | `read`    | `POST` requires `write`             |
| `/browser`         | `write`   | `GET` requires `read`               |
| `/memory`          | `read`    | `POST` requires `write`, `DELETE` requires `delete` |
| `/heartbeat`       | `read`    | `POST` requires `write`             |

Routes that don't match any prefix default to `read`.

## API Key Authentication

Tokens can be provided in two ways:

### Bearer Header

```
Authorization: Bearer sk-prod-abc123
```

### Query Parameter

```
GET /metrics?api_key=sk-prod-abc123
```

### Multiple Keys

Configure multiple keys with different levels to separate concerns:

```json
"apiKeys": [
  { "key": "sk-admin-secret",  "name": "ops-team",   "level": "admin" },
  { "key": "sk-write-abc",     "name": "api-client",  "level": "write" },
  { "key": "sk-read-dash",     "name": "dashboard",   "level": "read"  }
]
```

### Backwards Compatibility

When `apiKeys` is empty or omitted, authentication is completely disabled.
All requests are treated as `admin` level — this preserves the pre-0.3.1
behavior.

## Production Hardening

- Bind `host` to `127.0.0.1` and put the gateway behind a reverse proxy, or enable authentication before binding to `0.0.0.0`.
- Configure at least one API key; do not ship with `apiKeys: []`.
- Enable sandbox mode for `browser` and `shell` unless explicitly needed.
- Restrict CORS to known origins; avoid `*` in production.
- Terminate TLS at the edge and forward only from trusted networks.

## Sandbox Mode

Sandbox mode blocks specific dangerous routes regardless of the caller's
permission level. Enable it per category:

```json
{
  "httpGateway": {
    "apiKeys": [ ... ],
    "sandbox": {
      "browser": true,
      "shell": true
    }
  }
}
```

### Browser Sandbox

When `browser: true`, these routes return `403 Blocked by sandbox policy`:

| Blocked Route          | Description                |
|------------------------|----------------------------|
| `/browser/evaluate`    | Read-only browser query    |
| `/browser/navigate`    | Navigate to a URL          |
| `/browser/click`       | Click an element           |
| `/browser/type`        | Type into an element       |
| `/browser/close`       | Close the browser session  |

Read-only browser routes remain accessible:
- `GET /browser/status`
- `GET /browser/content`
- `GET /browser/links`

### Shell Sandbox

When `shell: true`, these routes are blocked:

| Blocked Route | Description              |
|---------------|--------------------------|
| `/daemon`     | System daemon management |

## Configuration Reference

Full `httpGateway` section:

```json
{
  "httpGateway": {
    "enabled": true,
    "port": 8080,
    "host": "127.0.0.1",
    "apiKeys": [
      { "key": "sk-your-secret-key", "name": "default", "level": "admin" }
    ],
    "sandbox": {
      "browser": false,
      "shell": false
    }
  }
}
```

Defaults (from `HTTP_GATEWAY_DEFAULTS` in `src/config.js`):

```js
{
  enabled: true,
  port: 8080,
  host: '127.0.0.1',
  apiKeys: [],       // empty = auth disabled
  allowBrowserEvaluate: false, // default-off; enables read-only /browser/evaluate
  sandbox: null,     // null = no restrictions
}
```

## HTTP API Examples

### Health check (no auth required)

```bash
curl http://localhost:8080/health
# {"status":"ok","uptime":"2h 15m"}
```

### Authenticated request

```bash
curl -H "Authorization: Bearer sk-prod-abc123" \
     http://localhost:8080/metrics
# {"uptime":"2h 15m","uptimeMs":8100000,"totals":{...}}
```

### Query param auth

```bash
curl "http://localhost:8080/metrics?api_key=sk-prod-abc123"
```

### Unauthenticated request (401)

```bash
curl http://localhost:8080/metrics
# {"error":"Authentication required","hint":"Provide Authorization: Bearer <key> header or ?api_key=<key> query param"}
```

### Insufficient permissions (403)

```bash
# read-level key trying to POST /memory/save
curl -X POST \
     -H "Authorization: Bearer sk-read-xyz789" \
     -H "Content-Type: application/json" \
     -d '{"summary":"test"}' \
     http://localhost:8080/memory/save
# {"error":"Forbidden","reason":"Route POST /memory/save requires 'write' permission (your level: 'read')"}
```

### Sandbox blocked (403)

```bash
# admin key, but browser sandbox is enabled
curl -X POST \
     -H "Authorization: Bearer sk-prod-abc123" \
     -H "Content-Type: application/json" \
     -d '{"expression":"document.title"}' \
     http://localhost:8080/browser/evaluate
# {"error":"Blocked by sandbox policy","reason":"Route '/browser/evaluate' is blocked by browser sandbox"}
```

### Evaluate disabled by default (403)

```bash
curl -X POST \
     -H "Authorization: Bearer sk-prod-abc123" \
     -H "Content-Type: application/json" \
     -d '{"expression":"document.title"}' \
     http://localhost:8080/browser/evaluate
# {"error":"Forbidden","reason":"Route /browser/evaluate is disabled by default. Set httpGateway.allowBrowserEvaluate=true to enable read-only evaluation."}
```
