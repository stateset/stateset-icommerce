# Heartbeat Monitor

The heartbeat monitor runs periodic commerce health checks and emits alerts
through the EventBridge, enabling proactive notifications across all connected
channels (Telegram, Discord, Slack, etc.) without any user prompting.

## Quick Start

Enable the heartbeat in your gateway config and turn on at least one check:

```json
{
  "heartbeat": {
    "enabled": true,
    "checks": [
      {
        "id": "low-stock",
        "name": "Low Stock",
        "checker": "low-stock",
        "intervalMs": 3600000,
        "enabled": true,
        "config": { "threshold": 10 }
      }
    ]
  }
}
```

The monitor starts automatically when the orchestrator boots. Alerts flow
through the EventBridge to all configured notification channels.

## Built-in Checkers

Six commerce checkers ship out of the box. All are disabled by default.

| ID                   | Description                              | Default Interval | Config Parameters              |
|----------------------|------------------------------------------|------------------|--------------------------------|
| `low-stock`          | Items below a stock threshold            | 1 hour           | `threshold` (default: 10)      |
| `abandoned-carts`    | Carts abandoned longer than a threshold  | 24 hours          | `minAgeHours` (default: 24)    |
| `revenue-milestone`  | Revenue hits a target for a period       | 1 hour           | `target` (default: 10000), `period` (default: "month") |
| `pending-returns`    | Pending returns older than a threshold   | 12 hours          | `maxAgeDays` (default: 7)      |
| `overdue-invoices`   | Any overdue invoices exist               | 24 hours          | none                           |
| `subscription-churn` | Cancelled or past-due subscriptions      | 24 hours          | none                           |

Each checker is resilient — if the commerce backend throws an error, the check
returns `triggered: false` with the error captured in `data.error`.

## Configuration

Full `heartbeat` section from the gateway config:

```json
{
  "heartbeat": {
    "enabled": false,
    "verbose": false,
    "checks": [
      { "id": "low-stock",          "name": "Low Stock",           "checker": "low-stock",          "intervalMs": 3600000,  "enabled": false, "config": { "threshold": 10 } },
      { "id": "abandoned-carts",    "name": "Abandoned Carts",     "checker": "abandoned-carts",    "intervalMs": 86400000, "enabled": false, "config": { "minAgeHours": 24 } },
      { "id": "revenue-milestone",  "name": "Revenue Milestone",   "checker": "revenue-milestone",  "intervalMs": 3600000,  "enabled": false, "config": { "target": 10000, "period": "month" } },
      { "id": "pending-returns",    "name": "Pending Returns",     "checker": "pending-returns",    "intervalMs": 43200000, "enabled": false, "config": { "maxAgeDays": 7 } },
      { "id": "overdue-invoices",   "name": "Overdue Invoices",    "checker": "overdue-invoices",   "intervalMs": 86400000, "enabled": false, "config": {} },
      { "id": "subscription-churn", "name": "Subscription Churn",  "checker": "subscription-churn", "intervalMs": 86400000, "enabled": false, "config": {} }
    ]
  }
}
```

Defaults (from `HEARTBEAT_DEFAULTS` in `src/config.js`):

```js
{
  enabled: false,
  verbose: false,
  checks: null,  // null = use built-in defaults (all disabled)
}
```

## HTTP API

All heartbeat routes require `read` level for GET and `write` level for POST.
Returns `501` when the heartbeat subsystem is not enabled.

### Get monitor status

```bash
curl -H "Authorization: Bearer $KEY" \
     http://localhost:8080/heartbeat/status
```

Response:

```json
{
  "running": true,
  "checkCount": 6,
  "enabledCount": 2,
  "checks": [
    {
      "id": "low-stock",
      "name": "Low Stock",
      "enabled": true,
      "lastRunAt": 1706540400000,
      "lastTriggeredAt": 1706540400000,
      "runCount": 5,
      "triggerCount": 2
    }
  ]
}
```

### List all checks

```bash
curl -H "Authorization: Bearer $KEY" \
     http://localhost:8080/heartbeat/checks
```

Response:

```json
{
  "checks": [
    { "id": "low-stock", "name": "Low Stock", "enabled": true, "runCount": 5, "triggerCount": 2 },
    { "id": "abandoned-carts", "name": "Abandoned Carts", "enabled": false, "runCount": 0, "triggerCount": 0 }
  ]
}
```

### Manually run a check

```bash
curl -X POST \
     -H "Authorization: Bearer $KEY" \
     http://localhost:8080/heartbeat/checks/low-stock/run
```

Response:

```json
{
  "checkId": "low-stock",
  "triggered": true,
  "data": {
    "items": [{ "sku": "WIDGET-001", "available": 3 }],
    "threshold": 10
  },
  "summary": "1 item(s) below 10 units"
}
```

### Enable a check

```bash
curl -X POST \
     -H "Authorization: Bearer $KEY" \
     http://localhost:8080/heartbeat/checks/low-stock/enable
```

Response:

```json
{ "checkId": "low-stock", "enabled": true }
```

### Disable a check

```bash
curl -X POST \
     -H "Authorization: Bearer $KEY" \
     http://localhost:8080/heartbeat/checks/low-stock/disable
```

Response:

```json
{ "checkId": "low-stock", "enabled": false }
```

## Event Pipeline

When a check triggers, the alert flows through the full notification pipeline:

```
HeartbeatMonitor
  └─ emits "alert" event
       └─ AutonomousEngine prefixes → "heartbeat:alert"
            └─ EventBridge maps to notification
                 └─ ChannelNotifier routes to channels
                      ├─ Telegram
                      ├─ Discord
                      ├─ Slack
                      └─ ... (all configured channels)
```

### Event Types

| Engine Event               | Notification Type     | Message Format                                    |
|----------------------------|-----------------------|---------------------------------------------------|
| `heartbeat:alert`          | `heartbeat.alert`     | `Heartbeat Alert [Low Stock]: 3 items below 10`   |
| `heartbeat:check:error`    | `heartbeat.error`     | `Heartbeat check error [low-stock]: DB timeout`    |

### Notification Routing

Route heartbeat alerts to specific channels in the `notifications` config:

```json
{
  "notifications": {
    "routes": {
      "heartbeat.alert": ["slack", "telegram"],
      "heartbeat.error": ["slack"]
    }
  }
}
```

## Custom Check Configuration

### Override thresholds

Change the low-stock threshold to 25 units:

```json
{
  "id": "low-stock",
  "name": "Low Stock",
  "checker": "low-stock",
  "intervalMs": 3600000,
  "enabled": true,
  "config": { "threshold": 25 }
}
```

### Change intervals

Check for abandoned carts every 6 hours instead of 24:

```json
{
  "id": "abandoned-carts",
  "name": "Abandoned Carts",
  "checker": "abandoned-carts",
  "intervalMs": 21600000,
  "enabled": true,
  "config": { "minAgeHours": 12 }
}
```

### Enable only specific checks

Pass a `checks` array with only the checks you want. Omitted checks won't
run at all:

```json
{
  "heartbeat": {
    "enabled": true,
    "checks": [
      { "id": "low-stock", "name": "Low Stock", "checker": "low-stock", "intervalMs": 3600000, "enabled": true, "config": { "threshold": 5 } },
      { "id": "overdue-invoices", "name": "Overdue Invoices", "checker": "overdue-invoices", "intervalMs": 43200000, "enabled": true, "config": {} }
    ]
  }
}
```

## Alert Format

The raw `heartbeat:alert` event payload:

```json
{
  "checkId": "low-stock",
  "checkName": "Low Stock",
  "data": {
    "items": [
      { "sku": "WIDGET-001", "available": 3 },
      { "sku": "GADGET-X",   "available": 0 }
    ],
    "threshold": 10
  },
  "summary": "2 item(s) below 10 units"
}
```

The EventBridge formats this into:

```
Heartbeat Alert [Low Stock]: 2 item(s) below 10 units
```

This message is delivered as plain text to all routed channels.
