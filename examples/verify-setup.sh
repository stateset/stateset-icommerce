#!/bin/bash
#
# StateSet iCommerce - Setup Verification Script
#
# Checks that all components are working correctly:
# - Sequencer connectivity
# - Database access
# - Sync configuration
# - Key registration
# - Basic CRUD operations
#
# Usage:
#   ./verify-setup.sh [--sequencer-url URL] [--db PATH]
#

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
SEQUENCER_URL="${STATESET_SEQUENCER_URL:-http://localhost:8080}"
DB_PATH="${STATESET_DB:-./store.db}"
VERBOSE=false
MIN_NODE_VERSION="20.20.0"
MIN_NPM_VERSION="10.0.0"

# Parse args
while [[ $# -gt 0 ]]; do
  case $1 in
    --sequencer-url) SEQUENCER_URL="$2"; shift 2 ;;
    --db) DB_PATH="$2"; shift 2 ;;
    -v|--verbose) VERBOSE=true; shift ;;
    --help)
      echo "Usage: $0 [options]"
      echo ""
      echo "Options:"
      echo "  --sequencer-url URL   Sequencer URL (default: $SEQUENCER_URL)"
      echo "  --db PATH             Database path (default: $DB_PATH)"
      echo "  -v, --verbose         Show detailed output"
      exit 0
      ;;
    *) shift ;;
  esac
done

# Counters
PASSED=0
FAILED=0
WARNINGS=0

# Test functions
pass() {
  echo -e "  ${GREEN}✓${NC} $1"
  ((PASSED++))
}

fail() {
  echo -e "  ${RED}✗${NC} $1"
  if [ -n "$2" ]; then
    echo -e "    ${RED}Error:${NC} $2"
  fi
  ((FAILED++))
}

warn() {
  echo -e "  ${YELLOW}!${NC} $1"
  ((WARNINGS++))
}

info() {
  if [ "$VERBOSE" = true ]; then
    echo -e "    ${BLUE}→${NC} $1"
  fi
}

version_gte() {
  local current="$1"
  local required="$2"
  local c_major=0 c_minor=0 c_patch=0
  local r_major=0 r_minor=0 r_patch=0

  IFS='.' read -r c_major c_minor c_patch _ <<< "$current"
  IFS='.' read -r r_major r_minor r_patch _ <<< "$required"

  c_minor="${c_minor:-0}"
  c_patch="${c_patch:-0}"
  r_minor="${r_minor:-0}"
  r_patch="${r_patch:-0}"

  if (( c_major > r_major )); then
    return 0
  fi
  if (( c_major < r_major )); then
    return 1
  fi
  if (( c_minor > r_minor )); then
    return 0
  fi
  if (( c_minor < r_minor )); then
    return 1
  fi
  (( c_patch >= r_patch ))
}

echo -e "${BLUE}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║          StateSet iCommerce - Setup Verification                ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════════╝${NC}"
echo ""

# =============================================================================
# 1. Prerequisites
# =============================================================================
echo -e "${YELLOW}[1/7] Checking prerequisites...${NC}"

# Check curl
if command -v curl &> /dev/null; then
  pass "curl is installed"
else
  fail "curl not found" "Install curl to continue"
fi

# Check jq (optional but useful)
if command -v jq &> /dev/null; then
  pass "jq is installed"
else
  warn "jq not installed (optional, but recommended)"
fi

# Check Node.js
if command -v node &> /dev/null; then
  NODE_VERSION_RAW=$(node --version)
  NODE_VERSION="${NODE_VERSION_RAW#v}"
  if version_gte "$NODE_VERSION" "$MIN_NODE_VERSION"; then
    pass "Node.js is installed ($NODE_VERSION_RAW)"
  else
    fail "Node.js $NODE_VERSION_RAW is too old" "Install Node.js $MIN_NODE_VERSION+"
  fi
else
  fail "Node.js not found" "Install Node.js 20.20.0+ (npm 10.0.0+)"
fi

# Check npm
if command -v npm &> /dev/null; then
  NPM_VERSION=$(npm --version)
  if version_gte "$NPM_VERSION" "$MIN_NPM_VERSION"; then
    pass "npm is installed ($NPM_VERSION)"
  else
    fail "npm $NPM_VERSION is too old" "Install npm $MIN_NPM_VERSION+"
  fi
else
  fail "npm not found" "Install npm 10.0.0+"
fi

# Check stateset CLI
if command -v stateset &> /dev/null; then
  pass "stateset CLI is installed"
else
  fail "stateset CLI not found" "Run: cd cli && npm install && npm link"
fi

# Check stateset-sync CLI
if command -v stateset-sync &> /dev/null; then
  pass "stateset-sync CLI is installed"
else
  warn "stateset-sync CLI not found (needed for sync features)"
fi

echo ""

# =============================================================================
# 2. Sequencer Connectivity
# =============================================================================
echo -e "${YELLOW}[2/7] Checking sequencer connectivity...${NC}"

# Health check
HEALTH_RESPONSE=$(curl -sf "$SEQUENCER_URL/health" 2>&1) || HEALTH_RESPONSE=""
if echo "$HEALTH_RESPONSE" | grep -qi "healthy\|ok"; then
  pass "Sequencer is healthy at $SEQUENCER_URL"
  info "Response: $HEALTH_RESPONSE"
elif [ -n "$HEALTH_RESPONSE" ]; then
  warn "Sequencer responded but may not be healthy"
  info "Response: $HEALTH_RESPONSE"
else
  fail "Cannot connect to sequencer at $SEQUENCER_URL" "Start with: docker-compose up -d"
fi

# Version check (if available)
VERSION_RESPONSE=$(curl -sf "$SEQUENCER_URL/version" 2>&1) || VERSION_RESPONSE=""
if [ -n "$VERSION_RESPONSE" ]; then
  info "Sequencer version: $VERSION_RESPONSE"
fi

echo ""

# =============================================================================
# 3. Database Access
# =============================================================================
echo -e "${YELLOW}[3/7] Checking database access...${NC}"

if [ -f "$DB_PATH" ]; then
  pass "Database file exists at $DB_PATH"
  DB_SIZE=$(du -h "$DB_PATH" | cut -f1)
  info "Database size: $DB_SIZE"
else
  warn "Database file not found at $DB_PATH"
  info "Will be created on first use"
fi

# Check if we can query the database
if command -v stateset &> /dev/null; then
  CUSTOMER_COUNT=$(stateset --db "$DB_PATH" "how many customers do we have?" 2>/dev/null | grep -oE '[0-9]+' | head -1) || CUSTOMER_COUNT=""
  if [ -n "$CUSTOMER_COUNT" ]; then
    pass "Database is readable ($CUSTOMER_COUNT customers)"
  else
    warn "Could not query database (may be empty)"
  fi
fi

echo ""

# =============================================================================
# 4. Sync Configuration
# =============================================================================
echo -e "${YELLOW}[4/7] Checking sync configuration...${NC}"

SYNC_CONFIG=".stateset/sync.json"
if [ -f "$SYNC_CONFIG" ]; then
  pass "Sync configuration exists"

  # Parse config
  if command -v jq &> /dev/null; then
    TENANT_ID=$(jq -r '.tenantId // .tenant_id // "unknown"' "$SYNC_CONFIG" 2>/dev/null)
    STORE_ID=$(jq -r '.storeId // .store_id // "unknown"' "$SYNC_CONFIG" 2>/dev/null)
    AGENT_ID=$(jq -r '.agentId // .agent_id // "unknown"' "$SYNC_CONFIG" 2>/dev/null)

    info "Tenant ID: $TENANT_ID"
    info "Store ID: $STORE_ID"
    info "Agent ID: $AGENT_ID"

    if [ "$TENANT_ID" != "unknown" ] && [ "$TENANT_ID" != "null" ]; then
      pass "Tenant ID configured"
    else
      fail "Tenant ID not configured"
    fi
  else
    info "Install jq for detailed config parsing"
  fi
else
  fail "Sync configuration not found" "Run: stateset-sync init ..."
fi

echo ""

# =============================================================================
# 5. Agent Keys
# =============================================================================
echo -e "${YELLOW}[5/7] Checking agent keys...${NC}"

KEYS_DIR=".stateset/keys"
if [ -d "$KEYS_DIR" ]; then
  pass "Keys directory exists"

  # Check for signing key
  if [ -f "$KEYS_DIR/signing.key" ] || [ -f "$KEYS_DIR/signing_private.key" ]; then
    pass "Signing key exists"
  else
    warn "Signing key not found"
  fi

  # Check for encryption key
  if [ -f "$KEYS_DIR/encryption.key" ] || [ -f "$KEYS_DIR/encryption_private.key" ]; then
    pass "Encryption key exists"
  else
    warn "Encryption key not found (optional)"
  fi
else
  warn "Keys directory not found"
  info "Run: stateset-sync keys:generate"
fi

echo ""

# =============================================================================
# 6. API Operations
# =============================================================================
echo -e "${YELLOW}[6/7] Testing API operations...${NC}"

if command -v stateset &> /dev/null; then
  # Test read operation
  READ_RESULT=$(stateset --db "$DB_PATH" "list products" 2>&1) || READ_RESULT=""
  if [ -n "$READ_RESULT" ] && ! echo "$READ_RESULT" | grep -qi "error"; then
    pass "Read operations working"
  else
    warn "Read operations may have issues"
    info "Response: ${READ_RESULT:0:100}"
  fi

  # Test write operation (create and immediately query)
  TEST_EMAIL="test-verify-$(date +%s)@example.com"
  WRITE_RESULT=$(stateset --db "$DB_PATH" --apply "create customer $TEST_EMAIL Test User" 2>&1) || WRITE_RESULT=""
  if echo "$WRITE_RESULT" | grep -qi "created\|success\|$TEST_EMAIL"; then
    pass "Write operations working"
  else
    warn "Write operations may have issues"
    info "Response: ${WRITE_RESULT:0:100}"
  fi
else
  warn "Skipping API tests (stateset CLI not available)"
fi

echo ""

# =============================================================================
# 7. Sync Operations
# =============================================================================
echo -e "${YELLOW}[7/7] Testing sync operations...${NC}"

if command -v stateset-sync &> /dev/null; then
  # Test sync status
  SYNC_STATUS=$(stateset-sync status 2>&1) || SYNC_STATUS=""
  if [ -n "$SYNC_STATUS" ] && ! echo "$SYNC_STATUS" | grep -qi "error\|failed"; then
    pass "Sync status check working"
    info "${SYNC_STATUS:0:100}"
  else
    warn "Sync status check had issues"
  fi

  # Test connection to sequencer via sync
  # This would typically do a lightweight ping
  info "Sync to sequencer: Use 'stateset-sync push' to test"
else
  warn "Skipping sync tests (stateset-sync CLI not available)"
fi

echo ""

# =============================================================================
# Summary
# =============================================================================
echo -e "${BLUE}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                    Verification Summary                        ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "  ${GREEN}Passed:${NC}   $PASSED"
echo -e "  ${RED}Failed:${NC}   $FAILED"
echo -e "  ${YELLOW}Warnings:${NC} $WARNINGS"
echo ""

if [ $FAILED -eq 0 ]; then
  echo -e "${GREEN}All critical checks passed!${NC}"
  echo ""
  echo "Your StateSet iCommerce setup is ready to use."
  echo ""
  echo "Next steps:"
  echo "  1. Seed demo data:  ./seed-demo-data.sh"
  echo "  2. Try the CLI:     stateset 'show me all products'"
  echo "  3. Sync events:     stateset-sync push"
  exit 0
else
  echo -e "${RED}Some checks failed. Please fix the issues above.${NC}"
  echo ""
  echo "Common fixes:"
  echo "  • Start sequencer: docker-compose -f docker-compose.full.yml up -d"
  echo "  • Initialize sync: stateset-sync init --sequencer-url $SEQUENCER_URL ..."
  echo "  • Generate keys:   stateset-sync keys:generate"
  exit 1
fi
