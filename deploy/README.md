# StateSet iCommerce Deployment Guide

This guide covers deploying StateSet iCommerce in various environments.

## Quick Start

### Local Development (Docker Compose)

```bash
# Start with SQLite (default)
docker-compose up -d stateset-cli

# Start with PostgreSQL
docker-compose --profile postgres up -d

# Start with full monitoring stack
docker-compose --profile postgres --profile monitoring up -d
```

### Kubernetes Deployment

```bash
# Apply all resources
kubectl apply -k deploy/kubernetes/

# Or apply individually
kubectl apply -f deploy/kubernetes/namespace.yaml
kubectl apply -f deploy/kubernetes/rbac.yaml
kubectl apply -f deploy/kubernetes/secrets.yaml  # Edit secrets first!
kubectl apply -f deploy/kubernetes/configmap.yaml
kubectl apply -f deploy/kubernetes/pvc.yaml
kubectl apply -f deploy/kubernetes/deployment.yaml
kubectl apply -f deploy/kubernetes/service.yaml
kubectl apply -f deploy/kubernetes/ingress.yaml
kubectl apply -f deploy/kubernetes/hpa.yaml
```

## Architecture Options

### Option 1: SQLite (Embedded) - Default

Best for:
- Single-instance deployments
- Edge/embedded scenarios
- Development and testing
- Low-traffic applications

```
┌─────────────────────────────────────┐
│           Application               │
│  ┌───────────────────────────────┐  │
│  │     StateSet iCommerce        │  │
│  │  ┌─────────────────────────┐  │  │
│  │  │   SQLite (embedded)     │  │  │
│  │  └─────────────────────────┘  │  │
│  └───────────────────────────────┘  │
└─────────────────────────────────────┘
```

Configuration:
```yaml
DATABASE_TYPE: "sqlite"
DATABASE_PATH: "/app/data/store.db"
```

### Option 2: PostgreSQL (Production)

Best for:
- Multi-instance deployments
- High availability requirements
- Heavy read/write workloads
- Production environments

```
┌──────────────────┐     ┌──────────────────┐
│   App Instance 1 │     │   App Instance 2 │
│   (StateSet)     │     │   (StateSet)     │
└────────┬─────────┘     └────────┬─────────┘
         │                        │
         └──────────┬─────────────┘
                    │
         ┌──────────▼──────────┐
         │     PostgreSQL      │
         │   (Primary/Replica) │
         └─────────────────────┘
```

Configuration:
```yaml
DATABASE_TYPE: "postgres"
POSTGRES_HOST: "stateset-postgres"
POSTGRES_PORT: "5432"
POSTGRES_DB: "stateset"
POSTGRES_USER: "stateset"
POSTGRES_PASSWORD: "<secure-password>"
```

## Resource Requirements

### Minimum (Development)
- CPU: 100m
- Memory: 256Mi
- Storage: 1Gi

### Recommended (Production)
- CPU: 1000m (1 core)
- Memory: 1Gi
- Storage: 50Gi (PostgreSQL)

### High-Traffic (Scaled)
- CPU: 2000m+ (2+ cores)
- Memory: 4Gi+
- Storage: 100Gi+ (PostgreSQL with replicas)

## Monitoring Setup

### Prometheus Metrics

StateSet exposes Prometheus metrics on port 9090 at `/metrics`:

```bash
# View metrics
curl http://localhost:9090/metrics
```

Key metrics:
- `stateset_orders_created_total` - Total orders created
- `stateset_orders_completed_total` - Total orders completed
- `stateset_inventory_available` - Current inventory by SKU
- `stateset_request_duration_seconds` - Request latency histogram
- `stateset_errors_total` - Error count by type

### Grafana Dashboards

Import dashboards from `deploy/grafana/provisioning/dashboards/`:

1. **Commerce Overview** - Orders, revenue, inventory health
2. **Performance** - Request latency, throughput, errors
3. **Inventory** - Stock levels, low stock alerts
4. **PostgreSQL** - Database performance (if using Postgres)

### Alerting

Alert rules are defined in `deploy/prometheus/alerts.yml`:

- High error rate (>10% of requests)
- High latency (p95 > 1s)
- Database connection issues
- Low inventory alerts
- Out of stock alerts
- Service unavailable

## Security Best Practices

### Secrets Management

**DO NOT** commit real secrets to the repository. Use:

1. **Kubernetes Secrets** (base64 encoded, not secure)
   ```bash
   kubectl create secret generic stateset-secrets \
     --from-literal=anthropic-api-key=sk-ant-xxx
   ```

2. **Sealed Secrets** (encrypted, GitOps-friendly)
   ```bash
   kubeseal --format=yaml < secrets.yaml > sealed-secrets.yaml
   ```

3. **External Secrets Operator** (AWS Secrets Manager, Vault, etc.)
   ```yaml
   apiVersion: external-secrets.io/v1beta1
   kind: ExternalSecret
   metadata:
     name: stateset-secrets
   spec:
     secretStoreRef:
       name: vault-backend
       kind: SecretStore
     target:
       name: stateset-secrets
     data:
       - secretKey: anthropic-api-key
         remoteRef:
           key: stateset/api-keys
           property: anthropic
   ```

### Network Policies

The included NetworkPolicy restricts:
- Ingress only from ingress controller and internal pods
- Egress only to DNS, internal pods, and external HTTPS (for API calls)

### Pod Security

Deployments use:
- Non-root user (UID 1000)
- Read-only root filesystem (where possible)
- Dropped all capabilities
- No privilege escalation

## Scaling

### Horizontal Pod Autoscaling

The HPA scales based on:
- CPU utilization (target: 70%)
- Memory utilization (target: 80%)

Configuration:
```yaml
minReplicas: 1
maxReplicas: 10
```

### Database Scaling

For PostgreSQL:
1. **Read Replicas** - Route read queries to replicas
2. **Connection Pooling** - Use PgBouncer for connection efficiency
3. **Partitioning** - Partition large tables (orders, transactions)

## Backup and Recovery

### SQLite Backup

```bash
# Copy database file
cp /app/data/store.db /backup/store-$(date +%Y%m%d).db

# Or use SQLite backup command
sqlite3 /app/data/store.db ".backup /backup/store-$(date +%Y%m%d).db"
```

### PostgreSQL Backup

```bash
# Logical backup
pg_dump -h stateset-postgres -U stateset -d stateset > backup.sql

# Point-in-time recovery (requires WAL archiving)
# Configure in postgresql.conf:
# archive_mode = on
# archive_command = 'cp %p /archive/%f'
```

## Troubleshooting

### Common Issues

1. **Database connection errors**
   ```bash
   # Check PostgreSQL status
   kubectl exec -it stateset-postgres-0 -- pg_isready

   # Check connection string
   kubectl get secret stateset-postgres-secrets -o jsonpath='{.data.database-url}' | base64 -d
   ```

2. **Out of memory**
   ```bash
   # Check resource usage
   kubectl top pods -n stateset

   # Increase limits in deployment.yaml
   ```

3. **Slow queries**
   ```bash
   # Enable query logging
   kubectl exec -it stateset-postgres-0 -- psql -U stateset -c "ALTER SYSTEM SET log_min_duration_statement = '100ms';"
   ```

### Health Checks

```bash
# Check CLI health
curl http://localhost:3000/health

# Check readiness
curl http://localhost:3000/ready

# Check metrics endpoint
curl http://localhost:9090/metrics
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `NODE_ENV` | Environment mode | `production` |
| `DATABASE_TYPE` | `sqlite` or `postgres` | `sqlite` |
| `DATABASE_PATH` | SQLite file path | `/app/data/store.db` |
| `POSTGRES_HOST` | PostgreSQL host | `localhost` |
| `POSTGRES_PORT` | PostgreSQL port | `5432` |
| `POSTGRES_DB` | PostgreSQL database | `stateset` |
| `POSTGRES_USER` | PostgreSQL user | `stateset` |
| `POSTGRES_PASSWORD` | PostgreSQL password | - |
| `LOG_LEVEL` | Log level | `info` |
| `LOG_FORMAT` | `json` or `text` | `json` |
| `METRICS_ENABLED` | Enable Prometheus metrics | `true` |
| `METRICS_PORT` | Metrics port | `9090` |
| `ANTHROPIC_API_KEY` | API key for Claude | - |

## Support

- GitHub Issues: https://github.com/stateset/stateset-icommerce/issues
- Documentation: https://docs.stateset.io
