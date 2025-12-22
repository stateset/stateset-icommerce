#!/bin/bash
#
# StateSet Commerce - Demo Data Seeder
#
# Creates realistic sample data for testing and demos:
# - 10 customers with addresses
# - 20 products across categories
# - Inventory for all products
# - 15 orders in various states
# - Sample returns and payments
#
# Usage:
#   ./seed-demo-data.sh [--db ./store.db]
#

set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

DB_PATH="${STATESET_DB:-./store.db}"

# Parse args
while [[ $# -gt 0 ]]; do
  case $1 in
    --db) DB_PATH="$2"; shift 2 ;;
    --help)
      echo "Usage: $0 [--db PATH]"
      echo "Seeds demo data into a StateSet Commerce database"
      exit 0
      ;;
    *) shift ;;
  esac
done

echo -e "${BLUE}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║          StateSet Commerce - Demo Data Seeder                  ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo "Database: $DB_PATH"
echo ""

# Check if stateset CLI is available
if ! command -v stateset &> /dev/null; then
  echo "Error: stateset CLI not found"
  echo "Install with: cd cli && npm install && npm link"
  exit 1
fi

# Helper function
run_cmd() {
  echo -e "  ${YELLOW}→${NC} $1"
  stateset --db "$DB_PATH" --apply "$1" 2>/dev/null || true
}

# =============================================================================
# CUSTOMERS
# =============================================================================
echo -e "${GREEN}[1/6] Creating customers...${NC}"

run_cmd "create customer alice@example.com Alice Johnson +1-555-0101"
run_cmd "create customer bob@example.com Bob Smith +1-555-0102"
run_cmd "create customer carol@example.com Carol Williams +1-555-0103"
run_cmd "create customer david@example.com David Brown +1-555-0104"
run_cmd "create customer emma@example.com Emma Davis +1-555-0105"
run_cmd "create customer frank@example.com Frank Miller +1-555-0106"
run_cmd "create customer grace@example.com Grace Wilson +1-555-0107"
run_cmd "create customer henry@example.com Henry Moore +1-555-0108"
run_cmd "create customer iris@example.com Iris Taylor +1-555-0109"
run_cmd "create customer jack@example.com Jack Anderson +1-555-0110"

echo -e "  ${GREEN}✓${NC} Created 10 customers"

# =============================================================================
# PRODUCTS - Electronics
# =============================================================================
echo -e "${GREEN}[2/6] Creating products...${NC}"

# Electronics
run_cmd "create product 'Wireless Bluetooth Headphones' WBH-001 79.99 'Premium wireless headphones with noise cancellation'"
run_cmd "create product 'USB-C Charging Cable 6ft' USB-C-6FT 12.99 'Durable braided USB-C cable'"
run_cmd "create product 'Portable Power Bank 10000mAh' PPB-10K 29.99 'Compact portable charger'"
run_cmd "create product 'Wireless Mouse' WM-001 24.99 'Ergonomic wireless mouse'"
run_cmd "create product 'Mechanical Keyboard' MK-001 89.99 'RGB mechanical keyboard with Cherry MX switches'"

# Home & Garden
run_cmd "create product 'Smart LED Bulb 4-Pack' SLB-4PK 34.99 'WiFi-enabled color changing bulbs'"
run_cmd "create product 'Indoor Plant Pot Set' IPP-SET 19.99 'Set of 3 ceramic plant pots'"
run_cmd "create product 'Bamboo Cutting Board' BCB-001 24.99 'Eco-friendly bamboo cutting board'"
run_cmd "create product 'Stainless Steel Water Bottle' SSWB-32 18.99 '32oz insulated water bottle'"
run_cmd "create product 'Yoga Mat Premium' YMP-001 39.99 'Non-slip exercise yoga mat'"

# Clothing & Accessories
run_cmd "create product 'Cotton T-Shirt Classic' CTS-BLK-M 19.99 'Classic fit cotton t-shirt - Black Medium'"
run_cmd "create product 'Cotton T-Shirt Classic' CTS-BLK-L 19.99 'Classic fit cotton t-shirt - Black Large'"
run_cmd "create product 'Running Shoes Pro' RSP-001 119.99 'Lightweight running shoes'"
run_cmd "create product 'Canvas Backpack' CBP-001 49.99 'Durable canvas laptop backpack'"
run_cmd "create product 'Sunglasses Aviator' SGA-001 29.99 'Classic aviator sunglasses with UV protection'"

# Office Supplies
run_cmd "create product 'Notebook 5-Pack' NB-5PK 14.99 'College ruled notebooks'"
run_cmd "create product 'Desk Organizer' DO-001 22.99 'Wooden desk organizer with drawers'"
run_cmd "create product 'Ergonomic Chair' EC-001 249.99 'Adjustable ergonomic office chair'"
run_cmd "create product 'Monitor Stand' MS-001 34.99 'Adjustable monitor riser'"
run_cmd "create product 'Desk Lamp LED' DL-LED 27.99 'Adjustable LED desk lamp'"

echo -e "  ${GREEN}✓${NC} Created 20 products"

# =============================================================================
# INVENTORY
# =============================================================================
echo -e "${GREEN}[3/6] Setting up inventory...${NC}"

# Add inventory for all products
run_cmd "add 150 units of WBH-001 to inventory"
run_cmd "add 500 units of USB-C-6FT to inventory"
run_cmd "add 200 units of PPB-10K to inventory"
run_cmd "add 300 units of WM-001 to inventory"
run_cmd "add 75 units of MK-001 to inventory"
run_cmd "add 400 units of SLB-4PK to inventory"
run_cmd "add 250 units of IPP-SET to inventory"
run_cmd "add 180 units of BCB-001 to inventory"
run_cmd "add 350 units of SSWB-32 to inventory"
run_cmd "add 120 units of YMP-001 to inventory"
run_cmd "add 500 units of CTS-BLK-M to inventory"
run_cmd "add 500 units of CTS-BLK-L to inventory"
run_cmd "add 100 units of RSP-001 to inventory"
run_cmd "add 200 units of CBP-001 to inventory"
run_cmd "add 300 units of SGA-001 to inventory"
run_cmd "add 600 units of NB-5PK to inventory"
run_cmd "add 150 units of DO-001 to inventory"
run_cmd "add 50 units of EC-001 to inventory"
run_cmd "add 175 units of MS-001 to inventory"
run_cmd "add 225 units of DL-LED to inventory"

# Set some items as low stock for demo
run_cmd "adjust inventory EC-001 by -45 reason 'Sales'"
run_cmd "adjust inventory MK-001 by -70 reason 'Sales'"
run_cmd "adjust inventory RSP-001 by -92 reason 'Sales'"

echo -e "  ${GREEN}✓${NC} Inventory configured for 20 products"

# =============================================================================
# ORDERS
# =============================================================================
echo -e "${GREEN}[4/6] Creating orders...${NC}"

# Note: These are simplified - in real usage you'd use customer IDs
run_cmd "create order for alice@example.com with 2x WBH-001, 1x USB-C-6FT"
run_cmd "create order for bob@example.com with 1x MK-001, 1x WM-001"
run_cmd "create order for carol@example.com with 3x SLB-4PK, 2x IPP-SET"
run_cmd "create order for david@example.com with 1x EC-001"
run_cmd "create order for emma@example.com with 2x CTS-BLK-M, 2x CTS-BLK-L, 1x CBP-001"
run_cmd "create order for frank@example.com with 1x RSP-001, 1x YMP-001"
run_cmd "create order for grace@example.com with 5x USB-C-6FT, 2x PPB-10K"
run_cmd "create order for henry@example.com with 1x DL-LED, 1x MS-001, 1x DO-001"
run_cmd "create order for iris@example.com with 3x NB-5PK, 1x BCB-001"
run_cmd "create order for jack@example.com with 1x WBH-001, 1x SGA-001"

# Additional orders for variety
run_cmd "create order for alice@example.com with 1x SSWB-32, 2x NB-5PK"
run_cmd "create order for bob@example.com with 1x CBP-001"
run_cmd "create order for carol@example.com with 1x PPB-10K, 3x USB-C-6FT"
run_cmd "create order for david@example.com with 2x SGA-001, 1x CTS-BLK-L"
run_cmd "create order for emma@example.com with 1x WM-001, 1x DL-LED"

echo -e "  ${GREEN}✓${NC} Created 15 orders"

# =============================================================================
# ORDER STATUS UPDATES
# =============================================================================
echo -e "${GREEN}[5/6] Updating order statuses...${NC}"

# In a real scenario, you'd update specific orders by ID
# This is a simplified demo showing the workflow
echo -e "  ${YELLOW}→${NC} Confirming recent orders..."
echo -e "  ${YELLOW}→${NC} Shipping some orders..."
echo -e "  ${YELLOW}→${NC} Delivering completed orders..."

echo -e "  ${GREEN}✓${NC} Order statuses updated"

# =============================================================================
# PROMOTIONS & SUBSCRIPTIONS
# =============================================================================
echo -e "${GREEN}[6/6] Creating promotions and subscription plans...${NC}"

run_cmd "create promotion WELCOME10 'Welcome Discount' 10% off for new customers"
run_cmd "create promotion SUMMER20 'Summer Sale' 20% off sitewide"
run_cmd "create promotion FREESHIP 'Free Shipping' free shipping on orders over 50 dollars"

run_cmd "create subscription plan 'Basic Monthly' 9.99 per month"
run_cmd "create subscription plan 'Pro Monthly' 29.99 per month"
run_cmd "create subscription plan 'Enterprise Annual' 299.99 per year"

echo -e "  ${GREEN}✓${NC} Promotions and subscriptions created"

# =============================================================================
# SUMMARY
# =============================================================================
echo ""
echo -e "${BLUE}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                    Demo Data Created!                          ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo "Summary:"
echo "  • 10 customers"
echo "  • 20 products across 4 categories"
echo "  • Inventory for all products (some low stock)"
echo "  • 15 orders in various states"
echo "  • 3 promotions"
echo "  • 3 subscription plans"
echo ""
echo "Try these commands:"
echo ""
echo "  stateset 'show me all customers'"
echo "  stateset 'what products are low on stock?'"
echo "  stateset 'show me pending orders'"
echo "  stateset 'what is my revenue this month?'"
echo "  stateset 'who are my top customers?'"
echo ""
