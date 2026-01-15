# Getting Started: StateSet CLI with Sequencer Sync

This guide walks you through setting up the StateSet CLI with the StateSet Sequencer for local-first commerce with verifiable event synchronization.

## Quick Start (TL;DR)

```bash
# 1. Generate secure credentials (do this once, save the values)
export ADMIN_API_KEY=$(openssl rand -hex 32)
export POSTGRES_PASSWORD=$(openssl rand -hex 16)
export STATESET_TENANT_ID=$(uuidgen)
export STATESET_STORE_ID=$(uuidgen)

# 2. Start everything with Docker
docker-compose -f docker-compose.full.yml up -d

# 3. Run automated setup
./setup-sync.sh --api-key $ADMIN_API_KEY --tenant-id $STATESET_TENANT_ID --store-id $STATESET_STORE_ID

# 4. Seed demo data
./seed-demo-data.sh

# 5. Verify everything works
./verify-setup.sh

# 6. Start using it!
stateset "show me all products"
stateset --apply "create order for alice@example.com with 2x WBH-001"
stateset-sync push
```

## Related Guides

- **[Workflows](./workflows.md)** - Step-by-step guides for common tasks (checkout, returns, inventory)
- **[Troubleshooting](./troubleshooting.md)** - Solutions to common problems
- **[Examples README](./README.md)** - Language-specific code examples

## Prerequisites

- Docker and Docker Compose (for running the sequencer)
- Node.js 18+ (for the CLI)
- The StateSet CLI (`@stateset/cli`) installed

## Architecture Overview

```
┌─────────────────────┐     ┌─────────────────────────┐
│  Local SQLite DB    │◄───►│  StateSet Sequencer     │
│  (store.db)         │     │  (Docker)               │
│                     │     │                         │
│  - Orders           │     │  - Event Ordering       │
│  - Customers        │     │  - Merkle Proofs        │
│  - Inventory        │     │  - Conflict Resolution  │
│  - Products         │     │  - Multi-tenant         │
└─────────────────────┘     └─────────────────────────┘
         ▲                            ▲
         │                            │
         │    stateset-sync           │
         └────────────────────────────┘
```

## Step 1: Generate Credentials & Start the Sequencer

First, generate secure credentials. **Save these values** - you'll need them throughout setup:

```bash
# Generate secure credentials (save these!)
export ADMIN_API_KEY=$(openssl rand -hex 32)
export POSTGRES_PASSWORD=$(openssl rand -hex 16)
export STATESET_TENANT_ID=$(uuidgen)
export STATESET_STORE_ID=$(uuidgen)

# Print them so you can save them
echo "ADMIN_API_KEY=$ADMIN_API_KEY"
echo "POSTGRES_PASSWORD=$POSTGRES_PASSWORD"
echo "STATESET_TENANT_ID=$STATESET_TENANT_ID"
echo "STATESET_STORE_ID=$STATESET_STORE_ID"
```

> **Tip:** Save these to a `.env` file (but don't commit it to git!):
> ```bash
> cat > .env << EOF
> ADMIN_API_KEY=$ADMIN_API_KEY
> POSTGRES_PASSWORD=$POSTGRES_PASSWORD
> STATESET_TENANT_ID=$STATESET_TENANT_ID
> STATESET_STORE_ID=$STATESET_STORE_ID
> EOF
> ```

Now start the StateSet Sequencer:

```bash
cd ~/stateset-sequencer
docker-compose up -d
```

Verify it's running:

```bash
curl http://localhost:8080/health
# Should return: {"status":"healthy"}
```

## Step 2: Register Your Tenant

Create a new tenant for your store:

```bash
# Generate UUIDs for tenant and store
TENANT_ID=$(uuidgen || cat /proc/sys/kernel/random/uuid)
STORE_ID=$(uuidgen || cat /proc/sys/kernel/random/uuid)

# Set your API key (from your sequencer setup)
API_KEY="your-api-key-here"

# Register the tenant with the sequencer
curl -X POST http://localhost:8080/admin/tenants \
  -H "X-API-Key: $API_KEY" \
  -H "Content-Type: application/json" \
  -d "{
    \"tenant_id\": \"$TENANT_ID\",
    \"name\": \"my-commerce-store\"
  }"
```

Expected response:

```json
{
  "tenant_id": "<your-generated-tenant-id>",
  "name": "my-commerce-store",
  "created_at": "2024-12-22T08:00:00Z"
}
```

## Step 3: Initialize StateSet Sync

Initialize your local database with sync capabilities:

```bash
cd ~/stateset-icommerce/cli

stateset-sync init \
  --sequencer-url http://localhost:8080 \
  --tenant-id $TENANT_ID \
  --store-id $STORE_ID \
  --api-key $API_KEY \
  --db ./store.db
```

This creates:
- `./store.db` - Your local SQLite commerce database
- `.stateset/sync.json` - Sync configuration
- `.stateset/keys/` - Cryptographic keys for signing events

## Step 4: Generate and Register Agent Keys

Generate Ed25519 signing keys for your agent:

```bash
stateset-sync keys:generate
```

This outputs your public key. Register it with the sequencer:

```bash
# Get your agent ID (generated during init)
AGENT_ID=$(cat .stateset/sync.json | jq -r '.agentId')

# Get your public key
PUBLIC_KEY=$(stateset-sync keys:export --format hex)

# Register the key with the sequencer
curl -X POST http://localhost:8080/v1/agents/keys \
  -H "X-API-Key: $API_KEY" \
  -H "Content-Type: application/json" \
  -d "{
    \"tenantId\": \"$TENANT_ID\",
    \"agentId\": \"$AGENT_ID\",
    \"keyId\": 1,
    \"publicKey\": \"$PUBLIC_KEY\"
  }"
```

Or use the CLI shortcut:

```bash
stateset-sync keys:register
```

## Step 5: Start Using the CLI

Now you can use the StateSet CLI for commerce operations:

### Create a Customer

```bash
stateset --apply "create customer alice@example.com Alice Smith"
```

### Create a Product

```bash
stateset --apply "create product 'Premium Widget' SKU-001 29.99"
```

### Create Inventory

```bash
stateset --apply "add 100 units of SKU-001 to inventory"
```

### Create an Order

```bash
stateset --apply "create order for alice@example.com with 2x SKU-001"
```

### Check Analytics

```bash
stateset "what's my revenue today?"
stateset "show me pending orders"
stateset "which products are low on stock?"
```

## Step 6: Sync Your Events

Push local events to the sequencer:

```bash
stateset-sync push
```

Pull remote events (from other agents):

```bash
stateset-sync pull
```

Check sync status:

```bash
stateset-sync status
```

## Complete Example Script

Here's a complete script to set everything up:

```bash
#!/bin/bash
set -e

# Configuration - SET THESE VALUES
SEQUENCER_URL="http://localhost:8080"
API_KEY="${STATESET_API_KEY:?Set STATESET_API_KEY}"
TENANT_ID="${STATESET_TENANT_ID:-$(uuidgen)}"
STORE_ID="${STATESET_STORE_ID:-$(uuidgen)}"
DB_PATH="./store.db"

echo "=== StateSet Commerce + Sequencer Setup ==="

# 1. Check sequencer is running
echo "Checking sequencer..."
curl -sf "$SEQUENCER_URL/health" > /dev/null || {
  echo "Error: Sequencer not running at $SEQUENCER_URL"
  echo "Start it with: cd ~/stateset-sequencer && docker-compose up -d"
  exit 1
}
echo "✓ Sequencer is healthy"

# 2. Register tenant (idempotent)
echo "Registering tenant..."
curl -sf -X POST "$SEQUENCER_URL/admin/tenants" \
  -H "X-API-Key: $API_KEY" \
  -H "Content-Type: application/json" \
  -d "{\"tenant_id\": \"$TENANT_ID\", \"name\": \"demo-store\"}" \
  > /dev/null 2>&1 || true
echo "✓ Tenant registered"

# 3. Initialize sync
echo "Initializing sync..."
stateset-sync init \
  --sequencer-url "$SEQUENCER_URL" \
  --tenant-id "$TENANT_ID" \
  --store-id "$STORE_ID" \
  --api-key "$API_KEY" \
  --db "$DB_PATH"
echo "✓ Sync initialized"

# 4. Generate and register keys
echo "Setting up agent keys..."
stateset-sync keys:generate 2>/dev/null || true
stateset-sync keys:register
echo "✓ Keys registered"

# 5. Create sample data
echo "Creating sample data..."
stateset --db "$DB_PATH" --apply "create customer demo@example.com Demo User"
stateset --db "$DB_PATH" --apply "create product 'Widget' WIDGET-001 29.99"
stateset --db "$DB_PATH" --apply "add 100 units of WIDGET-001"
echo "✓ Sample data created"

# 6. Push to sequencer
echo "Syncing with sequencer..."
stateset-sync push
echo "✓ Events pushed"

# 7. Show status
echo ""
echo "=== Setup Complete ==="
stateset-sync status
echo ""
stateset --db "$DB_PATH" "show me a summary"
```

## Multi-Agent Setup

To set up multiple agents syncing to the same store:

### Agent 1 (Primary)

```bash
# On machine 1
stateset-sync init \
  --sequencer-url http://sequencer.example.com:8080 \
  --tenant-id $TENANT_ID \
  --store-id $STORE_ID \
  --api-key $API_KEY \
  --db ./store.db

stateset-sync keys:generate
stateset-sync keys:register
```

### Agent 2 (Secondary)

```bash
# On machine 2
stateset-sync init \
  --sequencer-url http://sequencer.example.com:8080 \
  --tenant-id $TENANT_ID \
  --store-id $STORE_ID \
  --api-key $API_KEY \
  --db ./store.db

stateset-sync keys:generate
stateset-sync keys:register

# Pull existing events from sequencer
stateset-sync pull
```

### Create Encryption Group (for shared secrets)

```bash
# On Agent 1
stateset-sync groups:create --name "warehouse-agents"
stateset-sync groups:add-member --group-id $GROUP_ID --agent-id $AGENT_2_ID

# Now both agents can encrypt/decrypt shared events
```

## Key Management

### Rotate Keys (recommended every 30 days)

```bash
# Generate new keys
stateset-sync keys:rotate --all --register

# Check key expiration
stateset-sync keys:expiry
```

### Set Rotation Policy

```bash
# Auto-rotate signing keys every 30 days
stateset-sync keys:policy \
  --key-type signing \
  --max-age 720 \
  --grace-period 72
```

## Troubleshooting

### Check Sync Status

```bash
stateset-sync status
```

### View Sync History

```bash
stateset-sync history
```

### Verify Event Inclusion

```bash
stateset-sync verify <event-id>
```

### Handle Conflicts

```bash
# List conflicts
stateset-sync conflicts

# Resolve with remote-wins strategy
stateset-sync rebase --strategy remote-wins
```

### Debug Connection

```bash
# Test sequencer connectivity
curl -v http://localhost:8080/health

# Check your sync config
cat .stateset/sync.json
```

## Environment Variables

You can also configure via environment variables:

```bash
export STATESET_SEQUENCER_URL=http://localhost:8080
export STATESET_TENANT_ID=$(uuidgen)  # Generate once and save
export STATESET_STORE_ID=$(uuidgen)   # Generate once and save
export STATESET_API_KEY=your-api-key-here
export STATESET_DB=./store.db

# Now commands don't need flags
stateset-sync push
stateset-sync pull
```

## Available Scripts

| Script | Description |
|--------|-------------|
| `setup-sync.sh` | Automated setup (register tenant, init sync, generate keys) |
| `seed-demo-data.sh` | Create realistic demo data (customers, products, orders) |
| `verify-setup.sh` | Verify all components are working |
| `docker-compose.full.yml` | Full stack Docker setup |

### Usage

```bash
# Make scripts executable (if needed)
chmod +x *.sh

# Run setup
./setup-sync.sh --tenant-id YOUR_ID --store-id YOUR_ID

# Seed data
./seed-demo-data.sh --db ./store.db

# Verify setup
./verify-setup.sh --verbose
```

## Next Steps

- **[Common Workflows](./workflows.md)** - Checkout, returns, inventory, subscriptions
- **[Troubleshooting](./troubleshooting.md)** - Fix common issues
- [VES Protocol Specification](../docs/ves-protocol.md)
- [Key Management Guide](../docs/key-management.md)
- [Conflict Resolution Strategies](../docs/conflict-resolution.md)
- [Production Deployment](../docs/production-deployment.md)

---

For more help:

```bash
stateset --help
stateset-sync --help

# Or run verification
./verify-setup.sh
```
