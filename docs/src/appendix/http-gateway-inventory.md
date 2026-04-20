# HTTP Gateway Inventory

This page is generated from the built-in HTTP gateway route registry in `cli/src/channels/http-gateway.js`.
Do not edit it by hand. Regenerate it with:

```bash
node ./scripts/ci/generate_http_gateway_inventory.mjs
```

Machine-readable output lives at `artifacts/compatibility/http-gateway-inventory.json`.

## Summary

| Metric | Value |
| --- | --- |
| Total built-in routes | 44 |
| Permission levels | 5 |
| Tags | 12 |

## Permission Counts

| Permission | Routes |
| --- | --- |
| admin | 7 |
| delete | 1 |
| none | 4 |
| read | 15 |
| write | 17 |

## Tag Counts

| Tag | Routes |
| --- | --- |
| admin | 2 |
| agent | 2 |
| browser | 9 |
| commands | 1 |
| discovery | 2 |
| heartbeat | 5 |
| memory | 8 |
| observability | 1 |
| plugins | 3 |
| skills | 4 |
| system | 2 |
| voice | 5 |

## Built-in Routes

| Method | OpenAPI path | Permission | Tags | Summary |
| --- | --- | --- | --- | --- |
| `GET` | `/.well-known/service-info` | `none` | `discovery` | MPP service info |
| `DELETE` | `/agent/queue` | `admin` | `agent` | Clear agent queue lanes |
| `GET` | `/agent/queue` | `admin` | `agent` | Agent queue stats |
| `POST` | `/browser/click` | `write` | `browser` | Browser click |
| `POST` | `/browser/close` | `write` | `browser` | Close browser |
| `GET` | `/browser/content` | `read` | `browser` | Browser content |
| `POST` | `/browser/evaluate` | `admin` | `browser` | Evaluate browser expression |
| `GET` | `/browser/links` | `read` | `browser` | Browser links |
| `POST` | `/browser/navigate` | `write` | `browser` | Navigate browser |
| `POST` | `/browser/screenshot` | `write` | `browser` | Browser screenshot |
| `GET` | `/browser/status` | `read` | `browser` | Browser status |
| `POST` | `/browser/type` | `write` | `browser` | Browser type |
| `GET` | `/commands` | `read` | `commands` | List commands |
| `GET` | `/daemon` | `admin` | `admin` | Daemon status |
| `GET` | `/health` | `none` | `system` | Health check |
| `GET` | `/heartbeat/checks` | `read` | `heartbeat` | List heartbeat checks |
| `POST` | `/heartbeat/checks/{id}/disable` | `write` | `heartbeat` | Disable heartbeat check |
| `POST` | `/heartbeat/checks/{id}/enable` | `write` | `heartbeat` | Enable heartbeat check |
| `POST` | `/heartbeat/checks/{id}/run` | `write` | `heartbeat` | Run heartbeat check |
| `GET` | `/heartbeat/status` | `read` | `heartbeat` | Heartbeat status |
| `DELETE` | `/memory/{id}` | `delete` | `memory` | Delete memory |
| `POST` | `/memory/backfill` | `write` | `memory` | Backfill memories |
| `POST` | `/memory/hybrid-search` | `write` | `memory` | Hybrid search memory |
| `GET` | `/memory/recent/{channel}/{senderId}` | `read` | `memory` | Recent memories |
| `POST` | `/memory/save` | `write` | `memory` | Save memory |
| `POST` | `/memory/search` | `write` | `memory` | Search memory |
| `GET` | `/memory/stats` | `read` | `memory` | Memory stats |
| `POST` | `/memory/vector-search` | `write` | `memory` | Vector search memory |
| `GET` | `/metrics` | `read` | `observability` | Metrics summary |
| `GET` | `/openapi.json` | `none` | `discovery` | Gateway OpenAPI discovery |
| `GET` | `/plugins` | `read` | `plugins` | List plugins |
| `POST` | `/plugins/{id}/disable` | `admin` | `plugins` | Disable plugin |
| `POST` | `/plugins/{id}/enable` | `admin` | `plugins` | Enable plugin |
| `GET` | `/ready` | `none` | `system` | Readiness check |
| `GET` | `/remote-access` | `admin` | `admin` | Remote access status |
| `GET` | `/skills` | `read` | `skills` | List skills |
| `GET` | `/skills/{name}` | `read` | `skills` | Skill details |
| `GET` | `/skills/categories` | `read` | `skills` | Skill categories |
| `GET` | `/skills/marketplace` | `read` | `skills` | Marketplace catalog |
| `POST` | `/voice/session/disable/{sessionId}` | `write` | `voice` | Disable voice session |
| `POST` | `/voice/session/enable/{sessionId}` | `write` | `voice` | Enable voice session |
| `GET` | `/voice/status` | `read` | `voice` | Voice status |
| `POST` | `/voice/synthesize` | `write` | `voice` | Synthesize speech |
| `POST` | `/voice/transcribe` | `write` | `voice` | Transcribe audio |
