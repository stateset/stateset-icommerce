# Changelog

All notable changes to `@stateset/cli` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.7] - 2025-12-20

### Added

#### Payments API (5 tools)
Complete payment processing and refund management:
- `list_payments` - List all payments with filtering
- `get_payment` - Get payment details by ID
- `create_payment` - Create a payment for an order
- `complete_payment` - Mark payment as completed/captured
- `create_refund` - Process refunds for payments

#### Shipments API (3 tools)
Track shipments from warehouse to customer:
- `list_shipments` - List all shipments
- `create_shipment` - Create shipment with carrier and tracking info
- `deliver_shipment` - Mark shipment as delivered

#### Suppliers & Purchase Orders API (6 tools)
Full supply chain management:
- `list_suppliers` - List all suppliers
- `create_supplier` - Add new supplier with contact info
- `list_purchase_orders` - List all purchase orders
- `create_purchase_order` - Create PO for supplier
- `approve_purchase_order` - Approve PO for sending
- `send_purchase_order` - Send PO to supplier

#### Invoices API (5 tools)
B2B invoicing and accounts receivable:
- `list_invoices` - List all invoices
- `create_invoice` - Create invoice for customer
- `send_invoice` - Send invoice to customer
- `record_invoice_payment` - Record payment received on invoice
- `get_overdue_invoices` - Get overdue invoices for follow-up

#### Warranties API (4 tools)
Product warranty and claims management:
- `list_warranties` - List all warranties
- `create_warranty` - Create warranty for customer/product
- `create_warranty_claim` - File a warranty claim
- `approve_warranty_claim` - Approve warranty claim for processing

#### Manufacturing API (11 tools)
Bills of Materials and Work Order management:
- `list_boms` - List all Bills of Materials
- `get_bom` - Get BOM details with components
- `create_bom` - Create new BOM for a product
- `add_bom_component` - Add component/ingredient to BOM
- `activate_bom` - Activate BOM for production use
- `list_work_orders` - List manufacturing work orders
- `get_work_order` - Get work order details
- `create_work_order` - Create work order from BOM
- `start_work_order` - Start production on work order
- `complete_work_order` - Complete with quantity produced
- `cancel_work_order` - Cancel work order

### Fixed

- **Returns Schema**: Added missing `version` column to returns table in `012_versioning.sql` migration. This fixes the "column version does not exist" error when creating returns.
- **Invoice Payment Recording**: Fixed `record_invoice_payment` tool parameter name from `method` to `paymentMethod` to match the Rust binding interface. Fixed type conversion for amount field.
- **Warranty Creation**: Added required `customerId` parameter to `create_warranty` tool. Made `orderId` and `productId` optional to match the API contract.

### Changed

- Updated `TOOL_NAMES` array with all 34 new tool names for proper MCP registration
- Added new read-only tools to permission whitelist for safe preview mode operation
- Total MCP tools increased from 53 to **87 tools**

### Tool Count by Category

| Category | Tools |
|----------|-------|
| Customers | 3 |
| Orders | 6 |
| Products | 4 |
| Inventory | 6 |
| Returns | 5 |
| Carts/Checkout (ACP) | 14 |
| Analytics | 10 |
| Currency | 8 |
| Tax | 9 |
| Promotions | 10 |
| Subscriptions | 15 |
| Sync | 9 |
| Manufacturing | 11 |
| Payments | 5 |
| Shipments | 3 |
| Suppliers/POs | 6 |
| Invoices | 5 |
| Warranties | 4 |
| **Total** | **87** |

### Migration Notes

If upgrading from v0.1.6 or earlier with an existing database, run:

```sql
ALTER TABLE returns ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
```

## [0.1.2] - 2025-12-18

### Added

#### Storefront Creation Agent
- New `stateset-create` CLI command for scaffolding e-commerce websites
- Storefront agent with 13 scaffolding tools:
  - `create_project` - Initialize new storefront projects
  - `add_page` - Add pages (products, cart, checkout, account)
  - `add_component` - Add components (ProductCard, AddToCart, etc.)
  - `add_hook` - Add React hooks (useCart, useProducts, etc.)
  - `add_api_route` - Add API routes
  - `write_file` / `read_file` / `list_files` - File operations
  - `run_command` - Execute npm commands
  - `seed_database` - Create sample products
- Four project templates:
  - `nextjs` - Full-stack Next.js 14 with App Router, SSR, Tailwind
  - `nextjs-minimal` - Minimal Next.js setup
  - `vite-react` - Client-side SPA with WASM
  - `astro` - Static-first with Islands
- Comprehensive skill document with page, component, and hook templates
- Auto-routing to storefront agent for store creation requests

#### Observability & Telemetry
- New `telemetry.js` module with structured logging and tracing
- Distributed tracing with trace IDs and spans
- Tool call metrics with duration tracking
- Execution summary statistics
- `--verbose` flag for real-time telemetry output
- `--stats` flag to show execution statistics

#### Rich Output Formatting
- New `output.js` module for formatted CLI output
- ASCII table formatting with column alignment
- Progress bars and status indicators
- Currency, number, and date formatting
- Order/Cart/Customer card displays
- Color-coded status badges
- Consistent tool call formatting

#### Fine-Grained Permissions
- New `permissions.js` module for access control
- Five permission levels: `none`, `read`, `preview`, `write`, `admin`
- Per-tool permission mapping (56+ tools)
- Spending limits (max order value, daily totals)
- Rate limiting (tool calls/minute, write ops/minute)
- Confirmation thresholds for high-value operations
- Audit logging with sanitized parameters

#### Agent Improvements
- Enhanced agent routing with confidence scoring
- `routeToAgentWithConfidence()` returns confidence scores and alternatives
- Ambiguity detection for routing decisions

### Changed
- All CLI binaries now support `--verbose` flag
- Chat mode supports `/verbose on|off` command
- JSON output includes telemetry data when `--stats` is set
- Improved error messages with status icons

### Fixed
- Consistent version numbers across all CLI binaries

## [0.1.1] - 2025-12-17

### Added
- Initial release with core commerce agents
- Customer service, checkout, orders, inventory, returns, analytics agents
- 56 MCP tools for commerce operations
- Multi-currency support
- Interactive chat mode

## [0.1.0] - 2025-12-16

### Added
- Initial development release
