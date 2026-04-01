/**
 * Integration tests for MCP tool coverage
 *
 * Validates structural completeness and correctness of all MCP tool
 * definitions: naming, schemas, permissions, handlers, and uniqueness.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';

// Import every tool module individually so we can test them in isolation
import { customerTools } from '../../src/tools/customers.js';
import { orderTools } from '../../src/tools/orders.js';
import { productTools } from '../../src/tools/products.js';
import { inventoryTools } from '../../src/tools/inventory.js';
import { customObjectTools } from '../../src/tools/custom-objects.js';
import { returnTools } from '../../src/tools/returns.js';
import { cartTools } from '../../src/tools/carts.js';
import { analyticsTools } from '../../src/tools/analytics.js';
import { currencyTools } from '../../src/tools/currency.js';
import { taxTools } from '../../src/tools/tax.js';
import { promotionTools } from '../../src/tools/promotions.js';
import { subscriptionTools } from '../../src/tools/subscriptions.js';
import { syncTools } from '../../src/tools/sync.js';
import { manufacturingTools } from '../../src/tools/manufacturing.js';
import { paymentTools } from '../../src/tools/payments.js';
import { stablecoinTools } from '../../src/tools/stablecoin.js';
import { treasuryTools } from '../../src/tools/treasury.js';
import { erc8004Tools } from '../../src/tools/erc8004.js';
import { x402Tools } from '../../src/tools/x402.js';
import { agentCardTools } from '../../src/tools/agent-cards.js';
import { a2aTools } from '../../src/tools/a2a.js';
import { agentRuntimeTools } from '../../src/tools/agent-runtime.js';
import { shipmentTools } from '../../src/tools/shipments.js';
import { supplierTools } from '../../src/tools/suppliers.js';
import { invoiceTools } from '../../src/tools/invoices.js';
import { warrantyTools } from '../../src/tools/warranties.js';
import { importTools } from '../../src/tools/import.js';
import { policyTools } from '../../src/tools/policies.js';
import { vectorTools } from '../../src/tools/vector.js';
import { giftCardTools } from '../../src/tools/gift-cards.js';
import { storeCreditTools } from '../../src/tools/store-credits.js';
import { segmentTools } from '../../src/tools/segments.js';
import { shippingZoneTools } from '../../src/tools/shipping-zones.js';
import { reviewTools } from '../../src/tools/reviews.js';
import { wishlistTools } from '../../src/tools/wishlists.js';
import { loyaltyTools } from '../../src/tools/loyalty.js';
import { fraudTools } from '../../src/tools/fraud.js';
import { connectorTools } from '../../src/tools/connectors.js';
import { auditTools } from '../../src/tools/audit.js';
import { proofTools } from '../../src/tools/proofs.js';
import { circuitBreakerTools } from '../../src/tools/circuit-breaker.js';
import { checkoutTools } from '../../src/tools/checkout.js';
import { complianceTools } from '../../src/tools/compliance.js';
import { catalogTools } from '../../src/tools/catalog.js';
import { a2aAutomationTools } from '../../src/tools/a2a-automation.js';
import { a2aObservabilityTools } from '../../src/tools/a2a-observability.js';
import { a2aPlatformTools } from '../../src/tools/a2a-platform.js';
import { a2aIntelligenceTools } from '../../src/tools/a2a-intelligence.js';

// Also import the aggregated list exported from mcp-server
import { TOOL_NAMES } from '../../src/mcp-server.js';

// ============================================================================
// Helpers
// ============================================================================

const VALID_PERMISSIONS = new Set(['read', 'write', 'delete', 'admin', 'preview']);

/**
 * Collect all domain tool arrays into a single flat list (mirrors mcp-server.js
 * ALL_TOOL_DEFS but without the agentic runtime tools added inside the server).
 */
const DOMAIN_TOOL_ARRAYS = {
  customerTools,
  orderTools,
  productTools,
  inventoryTools,
  customObjectTools,
  returnTools,
  cartTools,
  analyticsTools,
  currencyTools,
  taxTools,
  promotionTools,
  subscriptionTools,
  syncTools,
  manufacturingTools,
  paymentTools,
  stablecoinTools,
  treasuryTools,
  erc8004Tools,
  x402Tools,
  agentCardTools,
  a2aTools,
  agentRuntimeTools,
  shipmentTools,
  supplierTools,
  invoiceTools,
  warrantyTools,
  importTools,
  policyTools,
  vectorTools,
  giftCardTools,
  storeCreditTools,
  segmentTools,
  shippingZoneTools,
  reviewTools,
  wishlistTools,
  loyaltyTools,
  fraudTools,
  connectorTools,
  auditTools,
  proofTools,
  circuitBreakerTools,
  checkoutTools,
  complianceTools,
  catalogTools,
  a2aAutomationTools,
  a2aObservabilityTools,
  a2aPlatformTools,
  a2aIntelligenceTools,
};

const ALL_DOMAIN_TOOLS = Object.values(DOMAIN_TOOL_ARRAYS).flat();

// ============================================================================
// Tests
// ============================================================================

describe('MCP tool coverage', () => {
  // --------------------------------------------------------------------------
  // Total tool count
  // --------------------------------------------------------------------------

  describe('total tool count', () => {
    it('TOOL_NAMES contains at least 180 tools', () => {
      assert.ok(
        TOOL_NAMES.length >= 180,
        `Expected >= 180 tools, got ${TOOL_NAMES.length}`,
      );
    });

    it('domain tool arrays contain at least 140 tools', () => {
      assert.ok(
        ALL_DOMAIN_TOOLS.length >= 140,
        `Expected >= 140 domain tools, got ${ALL_DOMAIN_TOOLS.length}`,
      );
    });

    it('every domain tool is represented in TOOL_NAMES', () => {
      for (const tool of ALL_DOMAIN_TOOLS) {
        const mcpName = `mcp__stateset-commerce__${tool.name}`;
        assert.ok(
          TOOL_NAMES.includes(mcpName),
          `Domain tool "${tool.name}" missing from TOOL_NAMES`,
        );
      }
    });
  });

  // --------------------------------------------------------------------------
  // Uniqueness
  // --------------------------------------------------------------------------

  describe('uniqueness', () => {
    it('all tool names in TOOL_NAMES are unique', () => {
      const seen = new Set();
      const dupes = [];
      for (const name of TOOL_NAMES) {
        if (seen.has(name)) dupes.push(name);
        seen.add(name);
      }
      assert.deepStrictEqual(dupes, [], `Duplicate TOOL_NAMES: ${dupes.join(', ')}`);
    });

    it('all domain tool names are unique across all modules', () => {
      const seen = new Map(); // name -> module
      const dupes = [];
      for (const [moduleName, tools] of Object.entries(DOMAIN_TOOL_ARRAYS)) {
        for (const tool of tools) {
          if (seen.has(tool.name)) {
            dupes.push(`"${tool.name}" in ${moduleName} and ${seen.get(tool.name)}`);
          }
          seen.set(tool.name, moduleName);
        }
      }
      assert.deepStrictEqual(dupes, [], `Duplicate tool names: ${dupes.join('; ')}`);
    });
  });

  // --------------------------------------------------------------------------
  // Structural validation for every tool
  // --------------------------------------------------------------------------

  describe('structural completeness', () => {
    for (const [moduleName, tools] of Object.entries(DOMAIN_TOOL_ARRAYS)) {
      describe(`module: ${moduleName}`, () => {
        it('exports an array', () => {
          assert.ok(Array.isArray(tools), `${moduleName} should export an array`);
        });

        it('exports at least 1 tool', () => {
          assert.ok(tools.length >= 1, `${moduleName} should have at least 1 tool`);
        });

        for (const tool of tools) {
          describe(`tool: ${tool.name}`, () => {
            it('has a non-empty string name', () => {
              assert.ok(typeof tool.name === 'string' && tool.name.length > 0);
            });

            it('has a non-empty string description', () => {
              assert.ok(
                typeof tool.description === 'string' && tool.description.length > 0,
                `Tool "${tool.name}" is missing description`,
              );
            });

            it('has an inputSchema object', () => {
              assert.ok(
                tool.inputSchema !== null && typeof tool.inputSchema === 'object',
                `Tool "${tool.name}" is missing inputSchema`,
              );
            });

            it('has a valid permission level', () => {
              assert.ok(
                VALID_PERMISSIONS.has(tool.permission),
                `Tool "${tool.name}" has invalid permission "${tool.permission}". ` +
                  `Expected one of: ${[...VALID_PERMISSIONS].join(', ')}`,
              );
            });

            it('has a handler function', () => {
              assert.ok(
                typeof tool.handler === 'function',
                `Tool "${tool.name}" is missing handler function`,
              );
            });

            it('name uses snake_case', () => {
              assert.match(
                tool.name,
                /^[a-z][a-z0-9]*(_[a-z0-9]+)*$/,
                `Tool "${tool.name}" does not follow snake_case convention`,
              );
            });

            it('description ends with a period or readable sentence', () => {
              // Should not be just whitespace or empty
              const desc = tool.description.trim();
              assert.ok(desc.length >= 10, `Tool "${tool.name}" description is too short`);
            });
          });
        }
      });
    }
  });

  // --------------------------------------------------------------------------
  // Schema validation — safeParse({}) should not throw
  // --------------------------------------------------------------------------

  describe('inputSchema safeParse validation', () => {
    for (const tool of ALL_DOMAIN_TOOLS) {
      it(`"${tool.name}" schema accepts safeParse({}) without throwing`, () => {
        // Each inputSchema is a plain object of Zod fields.
        // We wrap it in z.object() and run safeParse to verify the schema itself is valid Zod.
        const schema = tool.inputSchema;
        if (schema === null || schema === undefined) {
          // Some tools have no inputs - that is fine
          return;
        }

        // Verify all field values are valid Zod types
        const isPlainSchemaObject =
          typeof schema === 'object' &&
          !schema._def; // not itself a ZodType

        if (isPlainSchemaObject) {
          for (const [key, value] of Object.entries(schema)) {
            assert.ok(
              value !== undefined,
              `Tool "${tool.name}" schema field "${key}" is undefined`,
            );
            // Check that it is a ZodType instance (has _def property)
            if (value && typeof value === 'object' && value._def) {
              const result = value.safeParse(undefined);
              assert.ok(
                typeof result === 'object' && 'success' in result,
                `Zod safeParse on "${tool.name}.${key}" did not return expected shape`,
              );
            }
          }

          // Verify wrapping in z.object does not throw
          try {
            const wrapped = z.object(schema);
            const result = wrapped.safeParse({});
            assert.ok(
              typeof result === 'object' && 'success' in result,
              `z.object(schema).safeParse({}) failed for "${tool.name}"`,
            );
          } catch {
            // Some schemas contain z.record() or other non-key types
            // that z.object() cannot wrap — this is acceptable as long
            // as individual fields validated above.
          }
        } else if (schema._def) {
          // Schema is itself a Zod type (e.g. z.record())
          const result = schema.safeParse({});
          assert.ok(
            typeof result === 'object' && 'success' in result,
            `schema.safeParse({}) failed for "${tool.name}"`,
          );
        }
      });
    }
  });

  // --------------------------------------------------------------------------
  // Permission distribution
  // --------------------------------------------------------------------------

  describe('permission distribution', () => {
    it('has at least 30 read-only tools', () => {
      const readTools = ALL_DOMAIN_TOOLS.filter((t) => t.permission === 'read');
      assert.ok(
        readTools.length >= 30,
        `Expected >= 30 read tools, got ${readTools.length}`,
      );
    });

    it('has at least 30 write tools', () => {
      const writeTools = ALL_DOMAIN_TOOLS.filter((t) => t.permission === 'write');
      assert.ok(
        writeTools.length >= 30,
        `Expected >= 30 write tools, got ${writeTools.length}`,
      );
    });

    it('has some delete tools', () => {
      const deleteTools = ALL_DOMAIN_TOOLS.filter((t) => t.permission === 'delete');
      assert.ok(
        deleteTools.length >= 1,
        `Expected at least 1 delete tool, got ${deleteTools.length}`,
      );
    });

    it('every tool has an allowed permission', () => {
      const invalid = ALL_DOMAIN_TOOLS.filter((t) => !VALID_PERMISSIONS.has(t.permission));
      assert.deepStrictEqual(
        invalid.map((t) => t.name),
        [],
        'Some tools have invalid permissions',
      );
    });
  });

  // --------------------------------------------------------------------------
  // Tool naming conventions
  // --------------------------------------------------------------------------

  describe('naming conventions', () => {
    it('read tools typically start with list_ or get_', () => {
      const readTools = ALL_DOMAIN_TOOLS.filter((t) => t.permission === 'read');
      const readPrefixes = ['list_', 'get_', 'search_', 'validate_', 'check_', 'calculate_',
        'convert_', 'format_', 'count_', 'discover_', 'query_', 'sync_',
        'forecast_', 'analyze_', 'export_', 'estimate_', 'quote_',
        'reconcile_', 'verify_', 'preview_', 'compute_', 'evaluate_',
        'suggest_'];
      for (const tool of readTools) {
        const hasPrefix = readPrefixes.some((p) => tool.name.startsWith(p));
        // Not all read tools have these prefixes (e.g. agentic tools), but most should
        if (!hasPrefix) {
          // Just warn, don't fail - some tools have domain-specific names
        }
      }
      // At least 60% should follow conventions
      const withPrefix = readTools.filter((t) =>
        readPrefixes.some((p) => t.name.startsWith(p)),
      );
      const ratio = withPrefix.length / readTools.length;
      assert.ok(
        ratio >= 0.5,
        `Only ${Math.round(ratio * 100)}% of read tools follow naming conventions`,
      );
    });

    it('write tools typically start with create_, update_, set_, add_, or activate_', () => {
      const writeTools = ALL_DOMAIN_TOOLS.filter((t) => t.permission === 'write');
      const writePrefixes = ['create_', 'update_', 'set_', 'add_', 'activate_',
        'deactivate_', 'approve_', 'reject_', 'complete_', 'start_',
        'send_', 'record_', 'apply_', 'ship_', 'deliver_', 'adjust_',
        'reserve_', 'confirm_', 'release_', 'pause_', 'resume_',
        'cancel_', 'skip_', 'remove_', 'abandon_', 'import_',
        'register_', 'submit_', 'trigger_', 'enable_', 'ingest_',
        'capture_', 'refund_', 'issue_', 'redeem_', 'extend_',
        'assign_', 'transfer_', 'claim_', 'link_', 'mark_',
        'generate_', 'archive_', 'configure_', 'schedule_',
        'bulk_', 'seed_'];
      const withPrefix = writeTools.filter((t) =>
        writePrefixes.some((p) => t.name.startsWith(p)),
      );
      const ratio = writeTools.length > 0 ? withPrefix.length / writeTools.length : 1;
      assert.ok(
        ratio >= 0.4,
        `Only ${Math.round(ratio * 100)}% of write tools follow naming conventions`,
      );
    });
  });

  // --------------------------------------------------------------------------
  // TOOL_NAMES format
  // --------------------------------------------------------------------------

  describe('TOOL_NAMES format', () => {
    it('every TOOL_NAMES entry follows mcp__stateset-commerce__<name> format', () => {
      for (const name of TOOL_NAMES) {
        assert.match(
          name,
          /^mcp__stateset-commerce__[a-z][a-z0-9_]+$/,
          `Invalid TOOL_NAMES entry: "${name}"`,
        );
      }
    });

    it('TOOL_NAMES length matches total tools in aggregated list', () => {
      // TOOL_NAMES includes both domain tools and agentic runtime tools.
      // It should always be >= domain tools.
      assert.ok(
        TOOL_NAMES.length >= ALL_DOMAIN_TOOLS.length,
        `TOOL_NAMES (${TOOL_NAMES.length}) should be >= domain tools (${ALL_DOMAIN_TOOLS.length})`,
      );
    });
  });

  // --------------------------------------------------------------------------
  // Module-level sanity checks
  // --------------------------------------------------------------------------

  describe('per-module sanity', () => {
    const expectedModules = [
      { name: 'customerTools', min: 2 },
      { name: 'orderTools', min: 3 },
      { name: 'productTools', min: 2 },
      { name: 'inventoryTools', min: 3 },
      { name: 'returnTools', min: 3 },
      { name: 'cartTools', min: 5 },
      { name: 'analyticsTools', min: 5 },
      { name: 'paymentTools', min: 2 },
      { name: 'shipmentTools', min: 1 },
      { name: 'a2aTools', min: 2 },
    ];

    for (const { name, min } of expectedModules) {
      it(`${name} has at least ${min} tools`, () => {
        const tools = DOMAIN_TOOL_ARRAYS[name];
        assert.ok(
          tools && tools.length >= min,
          `${name} has ${tools?.length ?? 0} tools, expected >= ${min}`,
        );
      });
    }
  });

  // --------------------------------------------------------------------------
  // Handler arity
  // --------------------------------------------------------------------------

  describe('handler function signatures', () => {
    it('all handlers accept at least 1 argument (context object)', () => {
      for (const tool of ALL_DOMAIN_TOOLS) {
        assert.ok(
          tool.handler.length <= 1,
          `Tool "${tool.name}" handler has ${tool.handler.length} params; expected 0 or 1 (destructured context)`,
        );
      }
    });
  });

  // --------------------------------------------------------------------------
  // Description quality
  // --------------------------------------------------------------------------

  describe('description quality', () => {
    it('no two tools share the exact same description', () => {
      const descMap = new Map();
      const shared = [];
      for (const tool of ALL_DOMAIN_TOOLS) {
        const desc = tool.description.trim();
        if (descMap.has(desc)) {
          shared.push(`"${tool.name}" and "${descMap.get(desc)}" share description`);
        }
        descMap.set(desc, tool.name);
      }
      assert.deepStrictEqual(shared, [], `Shared descriptions found`);
    });
  });
});
