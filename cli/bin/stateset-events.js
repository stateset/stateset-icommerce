#!/usr/bin/env node

/**
 * StateSet iCommerce CLI - Event Streaming
 *
 * Usage:
 *   stateset-events                     Stream all events
 *   stateset-events --filter orders     Stream only order events
 *   stateset-events --filter inventory  Stream only inventory events
 *   stateset-events webhooks list       Manage webhooks
 */

import { parseArgs } from 'node:util';
import { RichOutput, ICONS } from '../src/claude-harness.js';
import { Commerce } from '@stateset/embedded';
import fs from 'node:fs/promises';

const HELP = `
StateSet iCommerce CLI - Event Streaming

USAGE:
  stateset-events [options]              Stream commerce events
  stateset-events webhooks <command>     Manage webhooks

OPTIONS:
  --db <path>        Path to SQLite database (default: ./store.db)
  --filter <type>    Filter events by type: orders, inventory, customers, products, returns
  --json             Output events as JSON (one per line)
  --output <file>    Write output to file (implies --json)
  --quiet, -q        Only output event data, no headers
  --help, -h         Show this help message

WEBHOOK COMMANDS:
  webhooks list                List registered webhooks
  webhooks add <url>           Add a webhook endpoint
  webhooks remove <id>         Remove a webhook
  webhooks test <id>           Send test event to webhook

FILTERS:
  orders      Order created, status changed, shipped, cancelled
  inventory   Stock adjusted, reserved, released, low stock alerts
  customers   Customer created, updated, status changed
  products    Product created, updated, variant changes
  returns     Return requested, approved, rejected, completed

EXAMPLES:
  # Stream all events in real-time
  stateset-events

  # Stream only order events
  stateset-events --filter orders

  # Stream as JSON for piping to other tools
  stateset-events --json | jq '.event_type'

  # Watch inventory for low stock alerts
  stateset-events --filter inventory --json

  # List webhooks
  stateset-events webhooks list

  # Add a webhook
  stateset-events webhooks add https://myapp.com/webhook --secret mysecret

  # Test a webhook
  stateset-events webhooks test abc-123
`;

const EVENT_ICONS = {
  OrderCreated: '📦',
  OrderStatusChanged: '🔄',
  OrderShipped: '🚚',
  OrderDelivered: '✅',
  OrderCancelled: '❌',
  CustomerCreated: '👤',
  CustomerUpdated: '✏️',
  InventoryAdjusted: '📊',
  InventoryReserved: '🔒',
  InventoryReservationReleased: '🔓',
  LowStockAlert: '⚠️',
  ProductCreated: '🏷️',
  ProductUpdated: '✏️',
  ReturnRequested: '↩️',
  ReturnApproved: '✅',
  ReturnRejected: '❌',
  ReturnCompleted: '📦',
  RefundIssued: '💰',
  default: '📣',
};

// Event type to filter category mapping
const FILTER_MAP = {
  orders: [
    'OrderCreated',
    'OrderStatusChanged',
    'OrderPaymentStatusChanged',
    'OrderFulfillmentStatusChanged',
    'OrderCancelled',
    'OrderItemAdded',
    'OrderItemRemoved',
  ],
  inventory: [
    'InventoryItemCreated',
    'InventoryAdjusted',
    'InventoryReserved',
    'InventoryReservationReleased',
    'InventoryReservationConfirmed',
    'LowStockAlert',
  ],
  customers: [
    'CustomerCreated',
    'CustomerUpdated',
    'CustomerStatusChanged',
    'CustomerAddressAdded',
  ],
  products: [
    'ProductCreated',
    'ProductUpdated',
    'ProductStatusChanged',
    'ProductVariantAdded',
    'ProductVariantUpdated',
  ],
  returns: [
    'ReturnRequested',
    'ReturnStatusChanged',
    'ReturnApproved',
    'ReturnRejected',
    'ReturnCompleted',
    'RefundIssued',
  ],
};

function getEventIcon(eventType) {
  return EVENT_ICONS[eventType] || EVENT_ICONS.default;
}

function formatEvent(event, output, isJson) {
  if (isJson) {
    return JSON.stringify(event);
  }

  const icon = getEventIcon(event.event_type);
  const timestamp = new Date(event.timestamp).toLocaleTimeString();
  const type = event.event_type;

  let details = '';
  if (event.order_id) details += ` order:${event.order_id.slice(0, 8)}`;
  if (event.customer_id) details += ` customer:${event.customer_id.slice(0, 8)}`;
  if (event.sku) details += ` sku:${event.sku}`;
  if (event.quantity !== undefined) details += ` qty:${event.quantity}`;
  if (event.amount !== undefined) details += ` $${event.amount}`;

  return `${output.dim(timestamp)} ${icon} ${output.bold(type)}${output.dim(details)}`;
}

async function streamEvents(commerce, filter, output, isJson, isQuiet, emit) {
  if (!isQuiet && !isJson) {
    console.log(`\n${ICONS.analytics} ${output.bold('Event Stream')}`);
    if (filter) {
      console.log(`   ${output.dim('Filter:')} ${filter}`);
    }
    console.log(`   ${output.dim('Press Ctrl+C to stop')}\n`);
    console.log(output.dim('─'.repeat(60)));
  }

  // Subscribe to events
  const events = commerce.events();
  let subscription;

  if (filter && FILTER_MAP[filter]) {
    // Use filtered subscription
    const allowedTypes = FILTER_MAP[filter];
    subscription = events.subscribe_filtered((event) => {
      return allowedTypes.includes(event.event_type);
    });
  } else {
    subscription = events.subscribe();
  }

  // Handle Ctrl+C gracefully
  process.on('SIGINT', () => {
    if (!isQuiet && !isJson) {
      console.log('\n' + output.dim('Stream ended'));
    }
    process.exit(0);
  });

  // Stream events
  try {
    while (true) {
      const event = await subscription.recv();
      if (!event) break;

      await emit(formatEvent(event, output, isJson));
    }
  } catch (error) {
    if (!isQuiet) {
      console.error(output.red(`Stream error: ${error.message}`));
    }
  }
}

async function handleWebhooks(command, args, commerce, output, isJson, emit) {
  switch (command) {
    case 'list': {
      const webhooks = commerce.list_webhooks();
      if (isJson) {
        await emit(JSON.stringify(webhooks, null, 2));
      } else {
        console.log(`\n${ICONS.session} ${output.bold('Registered Webhooks')}\n`);
        if (webhooks.length === 0) {
          console.log(output.dim('  No webhooks registered'));
        } else {
          for (const wh of webhooks) {
            const status = wh.active ? output.green('●') : output.red('○');
            console.log(`  ${status} ${output.bold(wh.name)}`);
            console.log(`     ${output.dim('ID:')} ${wh.id}`);
            console.log(`     ${output.dim('URL:')} ${wh.url}`);
            console.log(
              `     ${output.dim('Events:')} ${wh.event_types.length > 0 ? wh.event_types.join(', ') : 'all'}`,
            );
          }
        }
        console.log();
      }
      break;
    }

    case 'add': {
      const url = args[0];
      if (!url) {
        if (isJson) {
          await emit(JSON.stringify({ error: 'URL required' }));
        } else {
          console.error('Error: URL required');
          console.error('Usage: stateset-events webhooks add <url> [--secret <secret>]');
        }
        process.exit(1);
      }

      // Parse additional webhook options
      const secretIndex = args.indexOf('--secret');
      const secret = secretIndex >= 0 ? args[secretIndex + 1] : null;

      const eventsIndex = args.indexOf('--events');
      const eventTypes = eventsIndex >= 0 ? args[eventsIndex + 1].split(',') : [];

      if (isJson) {
        await emit(
          JSON.stringify({
            warning: 'Webhook management requires the events feature',
            url,
            secret: Boolean(secret),
            events: eventTypes,
          }),
        );
        break;
      }

      console.log(output.yellow('⚠️  Webhook management requires the events feature'));
      console.log(output.dim('   The webhook would be registered at: ' + url));
      if (secret) console.log(output.dim('   With HMAC secret configured'));
      break;
    }

    case 'remove': {
      const id = args[0];
      if (!id) {
        if (isJson) {
          await emit(JSON.stringify({ error: 'Webhook ID required' }));
        } else {
          console.error('Error: Webhook ID required');
          console.error('Usage: stateset-events webhooks remove <id>');
        }
        process.exit(1);
      }
      if (isJson) {
        await emit(
          JSON.stringify({ warning: 'Webhook management requires the events feature', id }),
        );
        break;
      }
      console.log(output.yellow('⚠️  Webhook management requires the events feature'));
      break;
    }

    case 'test': {
      const id = args[0];
      if (!id) {
        if (isJson) {
          await emit(JSON.stringify({ error: 'Webhook ID required' }));
        } else {
          console.error('Error: Webhook ID required');
          console.error('Usage: stateset-events webhooks test <id>');
        }
        process.exit(1);
      }
      if (isJson) {
        await emit(JSON.stringify({ warning: 'Webhook testing requires the events feature', id }));
        break;
      }
      console.log(output.yellow('⚠️  Webhook testing requires the events feature'));
      break;
    }

    default:
      if (isJson) {
        await emit(JSON.stringify({ error: `Unknown webhook command: ${command}` }));
      } else {
        console.error(`Unknown webhook command: ${command}`);
        console.error('Available commands: list, add, remove, test');
      }
      process.exit(1);
  }
}

async function main() {
  const { values, positionals } = parseArgs({
    options: {
      db: { type: 'string', default: './store.db' },
      filter: { type: 'string' },
      json: { type: 'boolean', default: false },
      output: { type: 'string' },
      quiet: { type: 'boolean', short: 'q', default: false },
      secret: { type: 'string' },
      events: { type: 'string' },
      help: { type: 'boolean', short: 'h', default: false },
    },
    allowPositionals: true,
  });

  if (values.help) {
    console.log(HELP);
    process.exit(0);
  }

  const outputPath = values.output || null;
  if (outputPath) {
    values.json = true;
  }

  const output = new RichOutput({ color: !values.json });
  const emit = async (line) => {
    if (outputPath) {
      await fs.appendFile(outputPath, line + '\n');
      return;
    }
    console.log(line);
  };
  const emitError = async (message) => {
    if (values.json) {
      await emit(JSON.stringify({ error: message }));
      return;
    }
    console.error(message);
  };

  if (outputPath) {
    await fs.writeFile(outputPath, '');
  }

  // Initialize commerce
  let commerce;
  try {
    commerce = new Commerce(values.db);
  } catch (error) {
    await emitError(`Database error: ${error.message}`);
    process.exit(1);
  }

  // Check for webhook subcommand
  if (positionals[0] === 'webhooks') {
    await handleWebhooks(positionals[1], positionals.slice(2), commerce, output, values.json, emit);
    return;
  }

  // Validate filter
  if (values.filter && !FILTER_MAP[values.filter]) {
    await emitError(`Unknown filter: ${values.filter}`);
    if (!values.json) {
      console.error(`Available filters: ${Object.keys(FILTER_MAP).join(', ')}`);
    }
    process.exit(1);
  }

  // Check if events feature is available
  if (typeof commerce.events !== 'function') {
    if (values.json) {
      await emit(JSON.stringify({ error: 'Event streaming requires the "events" feature' }));
    } else {
      console.error(output.red('Event streaming requires the "events" feature'));
      console.error(output.dim('Rebuild stateset-embedded with: cargo build --features events'));
    }
    process.exit(1);
  }

  // Stream events
  await streamEvents(commerce, values.filter, output, values.json, values.quiet, emit);
}

import { runMain } from '../src/graceful-shutdown.js';
runMain('stateset-events', main);
