# Changelog

All notable changes to `@stateset/cli` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
