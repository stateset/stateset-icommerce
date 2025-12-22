# StateSet Commerce - Troubleshooting Guide

Common issues and solutions for StateSet Commerce setup and operation.

## Table of Contents

- [Connection Issues](#connection-issues)
- [Database Issues](#database-issues)
- [Sync Issues](#sync-issues)
- [Key Management Issues](#key-management-issues)
- [CLI Issues](#cli-issues)
- [Docker Issues](#docker-issues)
- [Performance Issues](#performance-issues)
- [Error Reference](#error-reference)

---

## Connection Issues

### ❌ "Connection refused" to sequencer

**Symptoms:**
```
Error: connect ECONNREFUSED 127.0.0.1:8080
curl: (7) Failed to connect to localhost port 8080: Connection refused
```

**Causes & Solutions:**

1. **Sequencer not running**
   ```bash
   # Check if container is running
   docker ps | grep sequencer

   # Start the sequencer
   docker-compose -f docker-compose.full.yml up -d

   # Check logs
   docker logs stateset-sequencer
   ```

2. **Wrong port**
   ```bash
   # Verify sequencer URL
   echo $STATESET_SEQUENCER_URL

   # Test connection
   curl -v http://localhost:8080/health
   ```

3. **Firewall blocking**
   ```bash
   # Check if port is open
   sudo lsof -i :8080

   # On Linux, check firewall
   sudo ufw status
   ```

### ❌ "ETIMEDOUT" or slow connections

**Symptoms:**
```
Error: connect ETIMEDOUT
Request timed out after 30000ms
```

**Solutions:**

1. **Increase timeout**
   ```bash
   export STATESET_TIMEOUT=60000
   ```

2. **Check network latency**
   ```bash
   ping sequencer.example.com
   traceroute sequencer.example.com
   ```

3. **Check sequencer health**
   ```bash
   curl -w "\nTime: %{time_total}s\n" http://localhost:8080/health
   ```

### ❌ "401 Unauthorized" or "403 Forbidden"

**Symptoms:**
```
Error: Request failed with status 401
{"error": "Invalid API key"}
```

**Solutions:**

1. **Check API key**
   ```bash
   # Verify API key is set
   echo $STATESET_API_KEY

   # Test with explicit key
   curl -H "X-API-Key: dev_admin_key" http://localhost:8080/health
   ```

2. **Check sync config**
   ```bash
   cat .stateset/sync.json | jq '.apiKey'
   ```

3. **Regenerate API key** (if using production)
   ```bash
   # Contact admin or regenerate in dashboard
   ```

---

## Database Issues

### ❌ "Database is locked"

**Symptoms:**
```
Error: SQLITE_BUSY: database is locked
```

**Solutions:**

1. **Close other connections**
   ```bash
   # Find processes using the database
   lsof ./store.db

   # Kill hanging processes
   pkill -f "stateset.*store.db"
   ```

2. **Use WAL mode** (better concurrency)
   ```bash
   sqlite3 ./store.db "PRAGMA journal_mode=WAL;"
   ```

3. **Check for zombie processes**
   ```bash
   ps aux | grep stateset
   ```

### ❌ "No such table" or schema errors

**Symptoms:**
```
Error: no such table: customers
Error: table orders has no column named xyz
```

**Solutions:**

1. **Run migrations**
   ```bash
   # Initialize fresh database
   stateset --db ./store.db --apply "initialize database"

   # Or delete and recreate
   rm ./store.db
   stateset --db ./store.db "list products"  # Auto-creates schema
   ```

2. **Check database version**
   ```bash
   sqlite3 ./store.db "SELECT * FROM schema_migrations;"
   ```

### ❌ "Database file is corrupted"

**Symptoms:**
```
Error: database disk image is malformed
```

**Solutions:**

1. **Attempt recovery**
   ```bash
   # Dump what we can
   sqlite3 ./store.db ".dump" > backup.sql

   # Create new database
   sqlite3 ./store_new.db < backup.sql
   mv ./store_new.db ./store.db
   ```

2. **Restore from backup**
   ```bash
   cp ./backups/store.db.bak ./store.db
   ```

3. **Use integrity check**
   ```bash
   sqlite3 ./store.db "PRAGMA integrity_check;"
   ```

---

## Sync Issues

### ❌ "Sync config not found"

**Symptoms:**
```
Error: Sync configuration not found
Please run 'stateset-sync init' first
```

**Solutions:**

1. **Initialize sync**
   ```bash
   stateset-sync init \
     --sequencer-url http://localhost:8080 \
     --tenant-id YOUR_TENANT_ID \
     --store-id YOUR_STORE_ID \
     --api-key YOUR_API_KEY \
     --db ./store.db
   ```

2. **Check config exists**
   ```bash
   ls -la .stateset/
   cat .stateset/sync.json
   ```

### ❌ "Tenant not found"

**Symptoms:**
```
Error: Tenant a9db9387-... not found
```

**Solutions:**

1. **Register tenant**
   ```bash
   curl -X POST http://localhost:8080/admin/tenants \
     -H "X-API-Key: dev_admin_key" \
     -H "Content-Type: application/json" \
     -d '{"tenant_id": "YOUR_TENANT_ID", "name": "your-store"}'
   ```

2. **Verify tenant exists**
   ```bash
   curl -H "X-API-Key: dev_admin_key" \
     http://localhost:8080/admin/tenants/YOUR_TENANT_ID
   ```

### ❌ "Event signature invalid"

**Symptoms:**
```
Error: Event signature verification failed
Error: Invalid signature for event evt_abc123
```

**Solutions:**

1. **Re-register keys**
   ```bash
   stateset-sync keys:register
   ```

2. **Regenerate keys** (if corrupted)
   ```bash
   rm -rf .stateset/keys/
   stateset-sync keys:generate
   stateset-sync keys:register
   ```

3. **Check key matches**
   ```bash
   # Local public key
   stateset-sync keys:export --format hex

   # Registered key (from sequencer admin)
   curl -H "X-API-Key: dev_admin_key" \
     http://localhost:8080/v1/agents/YOUR_AGENT_ID/keys
   ```

### ❌ "Conflict detected"

**Symptoms:**
```
Warning: Conflict detected for event evt_abc123
Cannot apply remote event: conflicts with local state
```

**Solutions:**

1. **View conflicts**
   ```bash
   stateset-sync conflicts
   ```

2. **Resolve with strategy**
   ```bash
   # Accept remote changes
   stateset-sync rebase --strategy remote-wins

   # Keep local changes
   stateset-sync rebase --strategy local-wins

   # Manual resolution
   stateset-sync rebase --strategy manual
   ```

3. **Force sync** (use with caution)
   ```bash
   stateset-sync pull --force
   ```

---

## Key Management Issues

### ❌ "Key not found"

**Symptoms:**
```
Error: Signing key not found
Error: No key file at .stateset/keys/signing.key
```

**Solutions:**

1. **Generate keys**
   ```bash
   stateset-sync keys:generate
   ```

2. **Check key files**
   ```bash
   ls -la .stateset/keys/
   ```

### ❌ "Key expired"

**Symptoms:**
```
Warning: Signing key expires in 3 days
Error: Key has expired
```

**Solutions:**

1. **Rotate keys**
   ```bash
   stateset-sync keys:rotate --all --register
   ```

2. **Check expiration**
   ```bash
   stateset-sync keys:expiry
   ```

3. **Set rotation policy**
   ```bash
   stateset-sync keys:policy --key-type signing --max-age 720
   ```

### ❌ "Key already registered"

**Symptoms:**
```
Error: Key ID 1 already registered for this agent
```

**Solutions:**

1. **Use next key ID**
   ```bash
   stateset-sync keys:register --key-id 2
   ```

2. **Rotate to new key**
   ```bash
   stateset-sync keys:rotate --register
   ```

---

## CLI Issues

### ❌ "Command not found: stateset"

**Symptoms:**
```
bash: stateset: command not found
```

**Solutions:**

1. **Install CLI**
   ```bash
   cd ~/stateset-icommerce/cli
   npm install
   npm link
   ```

2. **Check PATH**
   ```bash
   echo $PATH
   which stateset

   # Add to PATH if needed
   export PATH="$PATH:$(npm bin -g)"
   ```

3. **Use npx**
   ```bash
   npx stateset "list products"
   ```

### ❌ "ANTHROPIC_API_KEY not set"

**Symptoms:**
```
Error: ANTHROPIC_API_KEY environment variable is required
```

**Solutions:**

1. **Set API key**
   ```bash
   export ANTHROPIC_API_KEY=sk-ant-...
   ```

2. **Add to shell profile**
   ```bash
   echo 'export ANTHROPIC_API_KEY=sk-ant-...' >> ~/.bashrc
   source ~/.bashrc
   ```

3. **Use .env file**
   ```bash
   echo "ANTHROPIC_API_KEY=sk-ant-..." > .env
   ```

### ❌ "Operation not permitted" without --apply

**Symptoms:**
```
This operation would modify data. Use --apply flag to execute.
Preview: Would create customer alice@example.com
```

**This is expected behavior!** The CLI requires `--apply` for write operations:

```bash
# Read operations (no flag needed)
stateset "list products"
stateset "show order ORD-123"

# Write operations (require --apply)
stateset --apply "create customer alice@example.com Alice"
stateset --apply "ship order ORD-123"
```

---

## Docker Issues

### ❌ "Container keeps restarting"

**Symptoms:**
```
stateset-sequencer    Restarting (1) 5 seconds ago
```

**Solutions:**

1. **Check logs**
   ```bash
   docker logs stateset-sequencer --tail 100
   ```

2. **Check dependencies**
   ```bash
   # Ensure postgres is healthy first
   docker logs stateset-postgres
   docker exec stateset-postgres pg_isready
   ```

3. **Check environment variables**
   ```bash
   docker inspect stateset-sequencer | jq '.[0].Config.Env'
   ```

### ❌ "Port already in use"

**Symptoms:**
```
Error: bind: address already in use
```

**Solutions:**

1. **Find process using port**
   ```bash
   sudo lsof -i :8080
   ```

2. **Kill process or use different port**
   ```bash
   # Kill process
   sudo kill -9 $(sudo lsof -t -i :8080)

   # Or change port in docker-compose.yml
   ports:
     - "8081:8080"
   ```

### ❌ "Cannot connect to Docker daemon"

**Symptoms:**
```
Cannot connect to the Docker daemon at unix:///var/run/docker.sock
```

**Solutions:**

1. **Start Docker**
   ```bash
   sudo systemctl start docker
   # or
   sudo service docker start
   ```

2. **Add user to docker group**
   ```bash
   sudo usermod -aG docker $USER
   # Log out and back in
   ```

---

## Performance Issues

### ❌ Slow queries

**Solutions:**

1. **Add indexes** (automatic in most cases)
   ```bash
   sqlite3 ./store.db "ANALYZE;"
   ```

2. **Vacuum database**
   ```bash
   sqlite3 ./store.db "VACUUM;"
   ```

3. **Check query plans**
   ```bash
   sqlite3 ./store.db "EXPLAIN QUERY PLAN SELECT * FROM orders WHERE customer_id = 'xyz';"
   ```

### ❌ High memory usage

**Solutions:**

1. **Limit cache size**
   ```bash
   sqlite3 ./store.db "PRAGMA cache_size = -10000;"  # 10MB
   ```

2. **Use streaming for large exports**
   ```bash
   stateset "export orders" --stream --format csv > orders.csv
   ```

### ❌ Sync taking too long

**Solutions:**

1. **Sync in batches**
   ```bash
   stateset-sync push --batch-size 100
   ```

2. **Check pending events**
   ```bash
   stateset-sync status
   ```

3. **Compress events**
   ```bash
   stateset-sync push --compress
   ```

---

## Error Reference

| Error Code | Meaning | Solution |
|------------|---------|----------|
| `ECONNREFUSED` | Cannot connect to server | Start the sequencer |
| `ETIMEDOUT` | Connection timed out | Check network, increase timeout |
| `SQLITE_BUSY` | Database is locked | Close other connections |
| `SQLITE_CORRUPT` | Database corrupted | Restore from backup |
| `401 Unauthorized` | Invalid API key | Check/regenerate API key |
| `403 Forbidden` | Not allowed | Check permissions |
| `404 Not Found` | Resource doesn't exist | Verify ID/path |
| `409 Conflict` | Sync conflict | Resolve conflicts |
| `422 Unprocessable` | Invalid input | Check request data |
| `500 Internal Error` | Server error | Check sequencer logs |

---

## Getting Help

If you're still stuck:

1. **Check logs**
   ```bash
   docker logs stateset-sequencer --tail 200
   ```

2. **Run diagnostics**
   ```bash
   ./verify-setup.sh -v
   ```

3. **Enable debug mode**
   ```bash
   DEBUG=stateset:* stateset "list products"
   ```

4. **File an issue**
   - [GitHub Issues](https://github.com/stateset/stateset-icommerce/issues)
   - Include: error message, logs, environment, steps to reproduce

5. **Community support**
   - [Discord](https://discord.gg/stateset)
   - [Discussions](https://github.com/stateset/stateset-icommerce/discussions)
