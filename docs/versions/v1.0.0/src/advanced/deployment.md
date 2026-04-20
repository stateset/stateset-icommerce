# Deployment

iCommerce is designed to run anywhere — from a single SQLite file on a laptop to a multi-instance PostgreSQL deployment behind a load balancer.

## Standalone (Tier 1)

The simplest deployment: a single process with an embedded SQLite database.

```bash
npm install -g @stateset/cli
stateset-init --quickstart
stateset "show me all customers"
```

No external services required. The entire commerce engine runs in-process.

### Production Hardening

For a production standalone deployment:

1. **Bind to localhost**: Set `host: "127.0.0.1"` in gateway config
2. **Enable API keys**: Configure at least one API key
3. **Use a reverse proxy**: Nginx or Caddy for TLS termination
4. **Enable policies**: Add YAML rules to `./policies/`
5. **Set up heartbeat**: Enable health checks for monitoring

## Docker

```dockerfile
FROM node:20-alpine

RUN npm install -g @stateset/cli
RUN stateset-init --quickstart

EXPOSE 8080 3000
CMD ["stateset-webhooks", "--port", "3000"]
```

```bash
docker build -t icommerce .
docker run -p 8080:8080 -p 3000:3000 -v ./data:/app/data icommerce
```

## PostgreSQL (Tier 2+)

For multi-instance deployments, switch to PostgreSQL:

### Rust

```rust
use stateset_embedded::AsyncCommerce;

let commerce = AsyncCommerce::connect(
    "postgres://user:pass@host/db?max_connections=25"
).await?;
```

### Configuration

```json
{
    "database": {
        "backend": "postgres",
        "url": "postgres://user:pass@host/db",
        "maxConnections": 25,
        "minConnections": 5
    }
}
```

See [Async vs Sync](../guides/async-vs-sync.md) for API differences.

## Kubernetes

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: icommerce
spec:
  replicas: 3
  template:
    spec:
      containers:
        - name: icommerce
          image: stateset/icommerce:1.0.0
          ports:
            - containerPort: 8080
          env:
            - name: DATABASE_URL
              valueFrom:
                secretKeyRef:
                  name: icommerce-secrets
                  key: database-url
          livenessProbe:
            httpGet:
              path: /health/live
              port: 8080
          readinessProbe:
            httpGet:
              path: /health/ready
              port: 8080
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `STATESET_DB_PATH` | SQLite database path | `./store.db` |
| `DATABASE_URL` | PostgreSQL connection string | (none) |
| `OPENAI_API_KEY` | For semantic search | (none) |
| `STATESET_LOG_LEVEL` | Log verbosity | `info` |

## Backup

### SQLite

```bash
# Online backup (safe while running)
sqlite3 store.db ".backup backup.db"
```

### PostgreSQL

```bash
pg_dump -h host -U user dbname > backup.sql
```

## Monitoring

- **Health**: `GET /health` — basic liveness
- **Metrics**: `GET /metrics` — uptime, operation counts
- **Heartbeat**: Periodic commerce health checks (see [Heartbeat Monitor](../guides/heartbeat.md))
