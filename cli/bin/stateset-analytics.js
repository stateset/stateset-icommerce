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
import { buildAgentOutputData, resolveOutputFormat, writeAgentOutputFile } from '../src/utils/agent-output.js';
import { resolveAgentRuntimeOptions, createStreamingHandler } from '../src/utils/agent-runtime.js';
import { DEFAULT_MODEL, CLI_VERSION } from '../src/config.js';
import { parseArgs } from 'node:util';

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
      model: { type: 'string', default: DEFAULT_MODEL },
      provider: { type: 'string' },
      think: { type: 'string', default: 'off' },
      stream: { type: 'boolean', default: false },
      budget: { type: 'string' },
      memory: { type: 'boolean', default: false },
      noMemory: { type: 'boolean', default: false },
      x402: { type: 'boolean', default: false },
      resume: { type: 'string' },
      json: { type: 'boolean', default: false },
      format: { type: 'string', default: 'table' },
      output: { type: 'string' },
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
    console.log(`@stateset/cli analytics-agent v${CLI_VERSION}`);
    process.exit(0);
  }

  const request = positionals.join(' ').trim();
  if (!request) {
    console.error('Error: No request provided');
    console.error('Usage: stateset-analytics "<your request>"');
    console.error('Run stateset-analytics --help for more information');
    process.exit(1);
  }

  const outputFormat = resolveOutputFormat({
    format: values.format,
    json: values.json,
    argv: process.argv
  });
  const isJsonOutput = outputFormat === 'json';

  if (values.stream && isJsonOutput) {
    console.error('Error: --stream cannot be used with JSON output. Remove --stream or use a non-JSON format.');
    process.exit(1);
  }

  let runtimeOptions;
  try {
    runtimeOptions = resolveAgentRuntimeOptions(values);
  } catch (error) {
    if (isJsonOutput) {
      console.log(JSON.stringify({ error: error.message }));
    } else {
      console.error(`\n❌ Error: ${error.message}`);
    }
    process.exit(1);
  }

  const { thinkLevel, providerName, streaming, maxBudgetUsd, memoryOverride, enableX402 } = runtimeOptions;

  if (!isJsonOutput) {
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
      thinkLevel,
      streaming,
      maxBudgetUsd,
      provider: providerName,
      enableMemory: memoryOverride === null ? null : memoryOverride,
      enableX402,
      onPartialMessage: createStreamingHandler(streaming),
      onToolCall: (toolCall) => {
        if (!isJsonOutput) {
          const toolName = toolCall.name.replace('mcp__stateset-commerce__', '');
          console.log(`🔧 ${toolName}(${JSON.stringify(toolCall.input)})`);
        }
      }
    });

    const outputData = buildAgentOutputData({
      agent: 'analytics',
      request,
      allowApply: false,
      result
    });

    if (values.output) {
      await writeAgentOutputFile(values.output, outputData, outputFormat);
      if (!isJsonOutput) {
        console.log(`Output written to ${values.output}`);
      }
    } else if (isJsonOutput) {
      console.log(JSON.stringify(outputData, null, 2));
    } else {
      if (streaming && result.response) {
        console.log();
      } else {
        console.log('\n' + result.response);
      }

      if (result.sessionId) {
        console.log(`\n💾 Session ID: ${result.sessionId}`);
        console.log(`   Use --resume ${result.sessionId} to continue this conversation`);
      }
    }

    process.exit(0);
  } catch (error) {
    if (isJsonOutput) {
      console.log(JSON.stringify({ error: error.message }));
    } else {
      console.error(`\n❌ Error: ${error.message}`);
    }
    process.exit(1);
  }
}

main();
