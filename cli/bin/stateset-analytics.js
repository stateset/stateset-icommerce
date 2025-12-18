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

import { runAgentLoop, AGENTS } from '../src/claude-harness.js';
import { parseArgs } from 'node:util';

const agentConfig = AGENTS['analytics'];

const HELP = `
StateSet Analytics Agent - Business Intelligence & Forecasting
${agentConfig?.description || 'Business intelligence and forecasting specialist'}

USAGE:
  stateset-analytics [options] "<request>"

OPTIONS:
  --db <path>        Path to SQLite database (default: ./store.db)
  --model <model>    Claude model to use (default: claude-sonnet-4-20250514)
  --resume <id>      Resume a previous session
  --json             Output as JSON
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

async function main() {
  const { values, positionals } = parseArgs({
    options: {
      db: { type: 'string', default: './store.db' },
      model: { type: 'string', default: 'claude-sonnet-4-20250514' },
      resume: { type: 'string' },
      json: { type: 'boolean', default: false },
      help: { type: 'boolean', short: 'h', default: false },
      version: { type: 'boolean', short: 'v', default: false }
    },
    allowPositionals: true
  });

  if (values.help) {
    console.log(HELP);
    process.exit(0);
  }

  if (values.version) {
    console.log('@stateset/cli analytics-agent v0.1.0');
    process.exit(0);
  }

  const request = positionals.join(' ').trim();
  if (!request) {
    console.error('Error: No request provided');
    console.error('Usage: stateset-analytics "<your request>"');
    console.error('Run stateset-analytics --help for more information');
    process.exit(1);
  }

  if (!values.json) {
    console.log(`\n📊 StateSet Analytics Agent`);
    console.log(`   Database: ${values.db}`);
    console.log(`   Mode: 👁️  Read-only (analytics)`);
    if (values.resume) {
      console.log(`   Session: ${values.resume}`);
    }
    console.log();
  }

  try {
    const result = await runAgentLoop({
      request,
      dbPath: values.db,
      model: values.model,
      allowApply: false, // Analytics is read-only
      resumeSessionId: values.resume,
      agent: 'analytics',
      onToolCall: (toolCall) => {
        if (!values.json) {
          const toolName = toolCall.name.replace('mcp__stateset-commerce__', '');
          console.log(`🔧 ${toolName}(${JSON.stringify(toolCall.input)})`);
        }
      }
    });

    if (values.json) {
      console.log(JSON.stringify({
        agent: 'analytics',
        request,
        sessionId: result.sessionId,
        response: result.response,
        toolResults: result.toolResults.map(tr => ({
          tool: tr.toolCall.name,
          input: tr.toolCall.input,
          result: tr.result
        }))
      }, null, 2));
    } else {
      console.log('\n' + result.response);

      if (result.sessionId) {
        console.log(`\n💾 Session ID: ${result.sessionId}`);
        console.log(`   Use --resume ${result.sessionId} to continue this conversation`);
      }
    }

    process.exit(0);
  } catch (error) {
    if (values.json) {
      console.log(JSON.stringify({ error: error.message }));
    } else {
      console.error(`\n❌ Error: ${error.message}`);
    }
    process.exit(1);
  }
}

main();
