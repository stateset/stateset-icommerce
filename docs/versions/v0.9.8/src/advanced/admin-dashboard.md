# Admin Dashboard

The iCommerce admin dashboard is a Next.js + TypeScript web application for managing commerce operations through a visual interface.

## Overview

The dashboard provides:

- Order, product, customer, and inventory management
- Analytics with revenue and inventory metrics
- Subscription and return processing workflows
- Agent status monitoring
- Gateway and payment configuration
- AI chat interface for natural language queries

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Framework | Next.js (App Router) |
| Language | TypeScript |
| Styling | Tailwind CSS |
| Testing | Vitest + jsdom |
| Commerce API | `@stateset/embedded` |

## App Structure

```
admin/src/app/
├── layout.tsx           # Root layout
├── page.tsx             # Dashboard home
├── analytics/           # Revenue and customer analytics
├── orders/              # Order management
├── products/            # Product catalog
├── customers/           # Customer management
├── inventory/           # Stock tracking
├── subscriptions/       # Subscription management
├── returns/             # Return processing
├── chat/                # AI chat interface
├── gateway/             # Payment gateway config
├── settings/            # Configuration
└── api/                 # API route handlers
```

## Components

| Component Group | Purpose |
|----------------|---------|
| `agents/` | Agent status and configuration |
| `analytics/` | Charts, metrics, and dashboards |
| `customers/` | Customer search and detail views |
| `finance/` | Financial reporting (A/P, A/R, P&L) |
| `gateway/` | Payment gateway setup |
| `inventory/` | Stock dashboards and alerts |
| `operations/` | Operational tools and workflows |
| `orders/` | Order list, detail, and fulfillment views |
| `products/` | Product editor and catalog views |
| `returns/` | Return request and RMA workflows |
| `subscriptions/` | Subscription management |
| `ui/` | Shared UI components (buttons, forms, tables, modals) |

## Shared Libraries

```
admin/src/lib/
├── shared/
│   ├── errors.ts           # Error types and codes
│   ├── response.ts         # Standardized API responses
│   ├── schemas.ts          # Zod validation schemas
│   ├── with-error-handler.ts  # API route error wrapper
│   └── request-context.ts  # Request metadata extraction
├── embedded.ts             # Commerce engine integration
├── gateway-client.ts       # Payment gateway API client
├── sessions-api.ts         # Multi-tenant session management
└── types.ts                # Global TypeScript definitions
```

## Running the Dashboard

```bash
cd admin
npm install
npm run dev
# → http://localhost:3000
```

## Testing

```bash
cd admin
npx vitest           # Run the admin test suite
npx vitest --ui      # Interactive test UI
npx vitest --coverage  # Coverage report
```

## Architecture

```
Browser                Admin (Next.js)         iCommerce Engine
  │                        │                        │
  │── Page request ──────►│                        │
  │                        │── Commerce API call ──►│
  │                        │   (via embedded.ts)    │
  │                        │◄── Result ────────────│
  │◄── Rendered page ─────│                        │
```

The dashboard connects to the embedded commerce engine via `admin/src/lib/embedded.ts`, which wraps the `@stateset/embedded` package. All operations go through the same permission and policy layers as the CLI.

## Key Screens

| Screen | Route | Capabilities |
|--------|-------|-------------|
| Dashboard | `/` | Revenue summary, order count, low-stock alerts |
| Orders | `/orders` | List, filter by status, view details, ship/cancel |
| Products | `/products` | Catalog browser, create/edit, variant management |
| Customers | `/customers` | Search, view history, manage segments |
| Inventory | `/inventory` | Stock levels, reservations, adjustment history |
| Returns | `/returns` | Pending RMAs, approve/reject, refund status |
| Subscriptions | `/subscriptions` | Active plans, billing status, churn metrics |
| Analytics | `/analytics` | Revenue charts, top products, cohort analysis |
| Chat | `/chat` | AI chat interface using the embedded toolkit |
| Settings | `/settings` | Configuration, API keys, adapter setup |

## AI Chat Interface

The `/chat` screen provides a natural language interface to the commerce engine. It uses the same embedded toolkit as the CLI, with the same policy and permission controls:

```
User: "What were our top 5 products last month?"
AI: Calls get_sales_summary + get_top_products tools → renders chart
```

## Deployment

```bash
# Development
cd admin && npm run dev    # http://localhost:3000

# Production build
cd admin && npm run build && npm start

# Docker
docker build -t icommerce-admin ./admin
docker run -p 3000:3000 -e DATABASE_PATH=./store.db icommerce-admin
```

## Customization

### Adding a Custom Page

1. Create `admin/src/app/my-page/page.tsx`
2. Add navigation in `admin/src/components/sidebar.tsx`
3. Use `admin/src/lib/embedded.ts` to call commerce APIs

### Adding a Dashboard Widget

Create a component in `admin/src/components/analytics/` and import it in the dashboard page.
