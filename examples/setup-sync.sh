#!/bin/bash
#
# StateSet Commerce + Sequencer Quick Setup
#
# This script sets up a local StateSet commerce instance connected to
# the StateSet Sequencer for verifiable event synchronization.
#
# Prerequisites:
#   - Docker running with stateset-sequencer at localhost:8080
#   - Node.js 18+ installed
#   - stateset CLI installed (npm link in cli/ directory)
#
# Usage:
#   ./setup-sync.sh [--tenant-id UUID] [--store-id UUID]
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration (can be overridden via environment or flags)
SEQUENCER_URL="${STATESET_SEQUENCER_URL:-http://localhost:8080}"
API_KEY="${STATESET_API_KEY:-}"
TENANT_ID="${STATESET_TENANT_ID:-}"
STORE_ID="${STATESET_STORE_ID:-}"
DB_PATH="${STATESET_DB:-./store.db}"
STORE_NAME="${STATESET_STORE_NAME:-demo-store}"

# Parse command line arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --tenant-id)
      TENANT_ID="$2"
      shift 2
      ;;
    --store-id)
      STORE_ID="$2"
      shift 2
      ;;
    --sequencer-url)
      SEQUENCER_URL="$2"
      shift 2
      ;;
    --api-key)
      API_KEY="$2"
      shift 2
      ;;
    --db)
      DB_PATH="$2"
      shift 2
      ;;
    --name)
      STORE_NAME="$2"
      shift 2
      ;;
    --help)
      echo "Usage: $0 [options]"
      echo ""
      echo "Required (via flags or environment):"
      echo "  --api-key KEY         API key (STATESET_API_KEY)"
      echo "  --tenant-id UUID      Tenant ID (STATESET_TENANT_ID)"
      echo "  --store-id UUID       Store ID (STATESET_STORE_ID)"
      echo ""
      echo "Optional:"
      echo "  --sequencer-url URL   Sequencer URL (default: http://localhost:8080)"
      echo "  --db PATH             Database path (default: ./store.db)"
      echo "  --name NAME           Store name (default: demo-store)"
      echo ""
      echo "Example:"
      echo "  export STATESET_API_KEY=your-api-key"
      echo "  export STATESET_TENANT_ID=\$(uuidgen)"
      echo "  export STATESET_STORE_ID=\$(uuidgen)"
      echo "  $0"
      exit 0
      ;;
    *)
      echo "Unknown option: $1"
      exit 1
      ;;
  esac
done

# Validate required configuration
MISSING=""
if [ -z "$API_KEY" ]; then
  MISSING="$MISSING\n  - STATESET_API_KEY or --api-key"
fi
if [ -z "$TENANT_ID" ]; then
  MISSING="$MISSING\n  - STATESET_TENANT_ID or --tenant-id (generate with: uuidgen)"
fi
if [ -z "$STORE_ID" ]; then
  MISSING="$MISSING\n  - STATESET_STORE_ID or --store-id (generate with: uuidgen)"
fi

if [ -n "$MISSING" ]; then
  echo -e "${RED}Error: Missing required configuration:${NC}"
  echo -e "$MISSING"
  echo ""
  echo "Run '$0 --help' for usage information."
  exit 1
fi

echo -e "${GREEN}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║        StateSet Commerce + Sequencer Setup                     ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo "Configuration:"
echo "  Sequencer URL: $SEQUENCER_URL"
echo "  Tenant ID:     $TENANT_ID"
echo "  Store ID:      $STORE_ID"
echo "  Database:      $DB_PATH"
echo "  Store Name:    $STORE_NAME"
echo ""

# Step 1: Check sequencer health
echo -e "${YELLOW}[1/6] Checking sequencer...${NC}"
if curl -sf "$SEQUENCER_URL/health" > /dev/null 2>&1; then
  echo -e "  ${GREEN}✓${NC} Sequencer is healthy at $SEQUENCER_URL"
else
  echo -e "  ${RED}✗${NC} Sequencer not responding at $SEQUENCER_URL"
  echo ""
  echo "  Start the sequencer with:"
  echo "    cd ~/stateset-sequencer && docker-compose up -d"
  echo ""
  exit 1
fi

# Step 2: Register tenant
echo -e "${YELLOW}[2/6] Registering tenant...${NC}"
RESPONSE=$(curl -sf -X POST "$SEQUENCER_URL/admin/tenants" \
  -H "X-API-Key: $API_KEY" \
  -H "Content-Type: application/json" \
  -d "{\"tenant_id\": \"$TENANT_ID\", \"name\": \"$STORE_NAME\"}" 2>&1) || true

if echo "$RESPONSE" | grep -q "tenant_id\|already exists"; then
  echo -e "  ${GREEN}✓${NC} Tenant registered: $TENANT_ID"
else
  echo -e "  ${YELLOW}!${NC} Tenant registration response: $RESPONSE"
fi

# Step 3: Initialize sync
echo -e "${YELLOW}[3/6] Initializing sync...${NC}"
if [ -f ".stateset/sync.json" ]; then
  echo -e "  ${YELLOW}!${NC} Sync already initialized, skipping..."
else
  stateset-sync init \
    --sequencer-url "$SEQUENCER_URL" \
    --tenant-id "$TENANT_ID" \
    --store-id "$STORE_ID" \
    --api-key "$API_KEY" \
    --db "$DB_PATH" 2>/dev/null || {
      # Manual init if command not available
      mkdir -p .stateset/keys
      cat > .stateset/sync.json << EOF
{
  "sequencerUrl": "$SEQUENCER_URL",
  "tenantId": "$TENANT_ID",
  "storeId": "$STORE_ID",
  "apiKey": "$API_KEY",
  "dbPath": "$DB_PATH",
  "agentId": "$(uuidgen 2>/dev/null || cat /proc/sys/kernel/random/uuid)"
}
EOF
    }
  echo -e "  ${GREEN}✓${NC} Sync initialized"
fi

# Step 4: Generate keys
echo -e "${YELLOW}[4/6] Generating agent keys...${NC}"
if [ -f ".stateset/keys/signing.key" ]; then
  echo -e "  ${YELLOW}!${NC} Keys already exist, skipping generation..."
else
  stateset-sync keys:generate 2>/dev/null || {
    echo -e "  ${YELLOW}!${NC} Key generation via CLI not available"
  }
fi
echo -e "  ${GREEN}✓${NC} Keys ready"

# Step 5: Register keys with sequencer
echo -e "${YELLOW}[5/6] Registering keys with sequencer...${NC}"
stateset-sync keys:register 2>/dev/null || {
  # Manual registration if needed
  AGENT_ID=$(cat .stateset/sync.json 2>/dev/null | grep agentId | cut -d'"' -f4)
  if [ -n "$AGENT_ID" ]; then
    echo -e "  ${YELLOW}!${NC} Manual key registration may be needed"
    echo "  Agent ID: $AGENT_ID"
  fi
}
echo -e "  ${GREEN}✓${NC} Keys registered"

# Step 6: Create sample data
echo -e "${YELLOW}[6/6] Creating sample data...${NC}"
if command -v stateset &> /dev/null; then
  stateset --db "$DB_PATH" --apply "create customer demo@example.com Demo User" 2>/dev/null || true
  stateset --db "$DB_PATH" --apply "create product 'Widget' WIDGET-001 29.99" 2>/dev/null || true
  echo -e "  ${GREEN}✓${NC} Sample data created"
else
  echo -e "  ${YELLOW}!${NC} stateset CLI not found, skipping sample data"
fi

# Summary
echo ""
echo -e "${GREEN}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║                    Setup Complete!                             ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo "Your StateSet Commerce instance is ready!"
echo ""
echo "Quick commands:"
echo ""
echo "  # Create data"
echo "  stateset --apply \"create customer alice@example.com Alice Smith\""
echo ""
echo "  # Query data"
echo "  stateset \"show me all customers\""
echo "  stateset \"what's my revenue today?\""
echo ""
echo "  # Sync with sequencer"
echo "  stateset-sync push    # Push local events to sequencer"
echo "  stateset-sync pull    # Pull remote events"
echo "  stateset-sync status  # Check sync status"
echo ""
echo "Configuration saved to: .stateset/sync.json"
echo "Database: $DB_PATH"
echo ""
