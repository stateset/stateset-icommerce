#!/usr/bin/env node

/**
 * StateSet Analytics Agent - Business intelligence and forecasting specialist
 *
 * Handles sales analytics, customer insights, inventory health, and forecasting.
 *
 * Usage:
 *   stateset-analytics "what's my best seller this month?"
 *   stateset-analytics "predict inventory needs for next month"
 */

import { AGENTS } from '../src/claude-harness.js';
import { runMain } from '../src/graceful-shutdown.js';
import { createAgentCliMain } from '../src/utils/agent-cli.js';

const agentConfig = AGENTS['analytics'];

const HELP = `
StateSet Analytics Agent - Business Intelligence & Forecasting
${agentConfig?.description || 'Business intelligence and forecasting specialist'}

USAGE:
  stateset-analytics [options] "<request>"

OPTIONS:
  --db <path>        Path to SQLite database (default: ./store.db)
  --model <model>    Claude model to use (default: see config.js)
  --provider <name>  Model provider (default: claude)
  --think <level>    Extended thinking: off, low, medium, high
  --stream           Stream partial responses
  --budget <usd>     Maximum spend per query in USD
  --memory           Enable memory
  --no-memory        Disable memory
  --x402             Enable x402 MCP tools (reads X402_* config/env)
  --resume <id>      Resume a previous session
  --json             Output as JSON
  --format <fmt>     Output format: table, json, csv, yaml (default: table)
  --output <file>    Write output to file
  --stats            Show execution stats and prompt budget
  --help, -h         Show this help message

TIME PERIODS:
  today              Current day
  last7days          Rolling 7 days
  last30days         Rolling 30 days (default)
  this_month         Current calendar month
  last_month         Previous calendar month
  this_year          Current year
  all_time           All historical data

AVAILABLE TOOLS:
  Sales Analytics:
  • get_sales_summary          - Revenue, orders, AOV, items sold
  • get_top_products           - Best sellers by revenue/units

  Customer Insights:
  • get_customer_metrics       - Total, new, returning customers
  • get_top_customers          - VIP customers by spend

  Inventory Intelligence:
  • get_inventory_health       - SKUs in stock, low stock, out of stock
  • get_low_stock_items        - Items needing attention

  Forecasting:
  • get_demand_forecast        - Predict future demand per SKU
  • get_revenue_forecast       - Predict future revenue

  Operations:
  • get_order_status_breakdown - Orders by status
  • get_return_metrics         - Return rate and refunds

EXAMPLES:
  # Sales performance
  stateset-analytics "what's my total revenue this month?"
  stateset-analytics "show me my best sellers"
  stateset-analytics "how's business doing compared to last month?"

  # Customer insights
  stateset-analytics "who are my top customers?"
  stateset-analytics "how many new customers did we get?"
  stateset-analytics "what's my customer retention rate?"

  # Inventory health
  stateset-analytics "what inventory needs attention?"
  stateset-analytics "show me items that are low in stock"
  stateset-analytics "which products are out of stock?"

  # Forecasting
  stateset-analytics "predict inventory needs for December"
  stateset-analytics "forecast revenue for next quarter"
  stateset-analytics "when will WIDGET-001 run out of stock?"

  # Operations
  stateset-analytics "how many orders are pending?"
  stateset-analytics "what's my return rate?"

NOTE:
  Analytics is read-only. No --apply flag needed.
  All queries analyze existing data without modifications.
`;

const main = createAgentCliMain({
  agent: 'analytics',
  commandName: 'stateset-analytics',
  title: 'StateSet Analytics Agent',
  icon: '📊',
  allowApply: false,
  modeLabel: '👁️  Read-only (analytics)',
  help: HELP,
});

runMain('stateset-analytics', main);
