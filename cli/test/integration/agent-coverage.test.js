/**
 * Integration tests for agent definitions coverage
 *
 * Validates that every agent has complete metadata, that all tool references
 * resolve to real MCP tools, and that no duplicates or structural issues exist.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { AGENTS } from '../../src/agent-definitions.js';
import { TOOL_NAMES } from '../../src/mcp-server.js';

// ============================================================================
// Helpers
// ============================================================================

const AGENT_ENTRIES = Object.entries(AGENTS);
const AGENT_KEYS = Object.keys(AGENTS);
const TOOL_NAME_SET = new Set(TOOL_NAMES);

// ============================================================================
// Tests
// ============================================================================

describe('Agent definitions coverage', () => {
  // --------------------------------------------------------------------------
  // Agent count
  // --------------------------------------------------------------------------

  describe('agent count', () => {
    it('has at least 15 agents defined', () => {
      assert.ok(
        AGENT_KEYS.length >= 15,
        `Expected >= 15 agents, got ${AGENT_KEYS.length}`,
      );
    });

    it('has the customer-service default agent', () => {
      assert.ok(
        'customer-service' in AGENTS,
        'customer-service agent should be defined',
      );
    });
  });

  // --------------------------------------------------------------------------
  // Uniqueness
  // --------------------------------------------------------------------------

  describe('uniqueness', () => {
    it('all agent keys are unique (object keys guarantee this)', () => {
      // Object keys are inherently unique, but let us also verify
      // that .name fields are unique across agents.
      const namesSeen = new Set();
      const dupes = [];
      for (const [key, agent] of AGENT_ENTRIES) {
        if (namesSeen.has(agent.name)) {
          dupes.push(`"${agent.name}" used by multiple agents (including "${key}")`);
        }
        namesSeen.add(agent.name);
      }
      assert.deepStrictEqual(dupes, [], `Duplicate agent names found`);
    });

    it('all agent keys are lowercase kebab-case or simple words', () => {
      for (const key of AGENT_KEYS) {
        assert.match(
          key,
          /^[a-z][a-z0-9]*(-[a-z0-9]+)*$/,
          `Agent key "${key}" does not follow kebab-case convention`,
        );
      }
    });
  });

  // --------------------------------------------------------------------------
  // Structural completeness for every agent
  // --------------------------------------------------------------------------

  describe('structural completeness', () => {
    for (const [key, agent] of AGENT_ENTRIES) {
      describe(`agent: ${key}`, () => {
        it('has a non-empty name string', () => {
          assert.ok(
            typeof agent.name === 'string' && agent.name.length > 0,
            `Agent "${key}" is missing name`,
          );
        });

        it('has a non-empty description string', () => {
          assert.ok(
            typeof agent.description === 'string' && agent.description.length > 0,
            `Agent "${key}" is missing description`,
          );
        });

        it('has a non-empty systemPrompt string', () => {
          assert.ok(
            typeof agent.systemPrompt === 'string' && agent.systemPrompt.length > 20,
            `Agent "${key}" is missing or has too-short systemPrompt`,
          );
        });

        it('has a tools array', () => {
          assert.ok(
            Array.isArray(agent.tools),
            `Agent "${key}" tools should be an array`,
          );
        });

        it('tools array is not empty', () => {
          assert.ok(
            agent.tools.length > 0,
            `Agent "${key}" should have at least 1 tool`,
          );
        });

        it('all tools are strings', () => {
          for (const tool of agent.tools) {
            assert.ok(
              typeof tool === 'string',
              `Agent "${key}" has non-string tool: ${typeof tool}`,
            );
          }
        });

        it('has no duplicate tools', () => {
          const seen = new Set();
          const dupes = [];
          for (const tool of agent.tools) {
            if (seen.has(tool)) dupes.push(tool);
            seen.add(tool);
          }
          assert.deepStrictEqual(
            dupes,
            [],
            `Agent "${key}" has duplicate tools: ${dupes.join(', ')}`,
          );
        });
      });
    }
  });

  // --------------------------------------------------------------------------
  // Tool reference validation
  // --------------------------------------------------------------------------

  describe('tool reference validation', () => {
    for (const [key, agent] of AGENT_ENTRIES) {
      // The storefront agent references scaffold tools which are on a separate
      // MCP server, so skip that agent for cross-reference checks.
      if (key === 'storefront') continue;

      describe(`agent: ${key}`, () => {
        it('all tool references exist in TOOL_NAMES', () => {
          const missing = agent.tools.filter((t) => !TOOL_NAME_SET.has(t));
          assert.deepStrictEqual(
            missing,
            [],
            `Agent "${key}" references unknown tools: ${missing.join(', ')}`,
          );
        });
      });
    }

    it('storefront agent references scaffold tools (separate server)', () => {
      const storefront = AGENTS.storefront;
      if (!storefront) return; // may not exist
      // Storefront tools reference mcp__stateset-scaffold__ prefix
      for (const tool of storefront.tools) {
        assert.ok(
          tool.startsWith('mcp__stateset-scaffold__') || tool.startsWith('mcp__stateset-commerce__'),
          `Storefront tool "${tool}" has unexpected prefix`,
        );
      }
    });
  });

  // --------------------------------------------------------------------------
  // System prompt quality
  // --------------------------------------------------------------------------

  describe('system prompt quality', () => {
    for (const [key, agent] of AGENT_ENTRIES) {
      describe(`agent: ${key}`, () => {
        it('system prompt mentions role or purpose', () => {
          const lower = agent.systemPrompt.toLowerCase();
          assert.ok(
            lower.includes('role') ||
              lower.includes('specialist') ||
              lower.includes('agent') ||
              lower.includes('you are') ||
              lower.includes('help'),
            `Agent "${key}" systemPrompt should describe the agent role`,
          );
        });

        it('system prompt mentions safety rules or apply flag', () => {
          const lower = agent.systemPrompt.toLowerCase();
          assert.ok(
            lower.includes('safety') ||
              lower.includes('--apply') ||
              lower.includes('preview') ||
              lower.includes('read-only') ||
              lower.includes('confirmation'),
            `Agent "${key}" systemPrompt should mention safety/apply`,
          );
        });

        it('system prompt is at least 100 characters', () => {
          assert.ok(
            agent.systemPrompt.length >= 100,
            `Agent "${key}" systemPrompt is only ${agent.systemPrompt.length} chars`,
          );
        });
      });
    }
  });

  // --------------------------------------------------------------------------
  // Tool coverage - customer-service should have all tools
  // --------------------------------------------------------------------------

  describe('customer-service agent coverage', () => {
    it('customer-service agent references all TOOL_NAMES', () => {
      const cs = AGENTS['customer-service'];
      assert.ok(cs, 'customer-service agent must exist');
      // customer-service uses TOOL_NAMES directly so should match exactly
      assert.strictEqual(
        cs.tools.length,
        TOOL_NAMES.length,
        `customer-service has ${cs.tools.length} tools, TOOL_NAMES has ${TOOL_NAMES.length}`,
      );
    });
  });

  // --------------------------------------------------------------------------
  // Specialized agents should have fewer tools than customer-service
  // --------------------------------------------------------------------------

  describe('specialized agents have scoped tool sets', () => {
    const specialized = AGENT_ENTRIES.filter(([key]) => key !== 'customer-service');

    for (const [key, agent] of specialized) {
      it(`${key} has fewer tools than customer-service`, () => {
        const cs = AGENTS['customer-service'];
        assert.ok(
          agent.tools.length < cs.tools.length,
          `Agent "${key}" has ${agent.tools.length} tools, should be < ${cs.tools.length}`,
        );
      });
    }
  });

  // --------------------------------------------------------------------------
  // Well-known agent expectations
  // --------------------------------------------------------------------------

  describe('well-known agents', () => {
    const expected = [
      'customer-service',
      'checkout',
      'orders',
      'inventory',
      'returns',
      'analytics',
      'promotions',
      'subscriptions',
      'payments',
      'shipments',
    ];

    for (const name of expected) {
      it(`"${name}" agent is defined`, () => {
        assert.ok(name in AGENTS, `Expected agent "${name}" to be defined`);
      });
    }
  });

  // --------------------------------------------------------------------------
  // Agent tool prefix consistency
  // --------------------------------------------------------------------------

  describe('tool prefix consistency', () => {
    for (const [key, agent] of AGENT_ENTRIES) {
      it(`${key} tools all have valid MCP prefixes`, () => {
        for (const tool of agent.tools) {
          assert.ok(
            tool.startsWith('mcp__'),
            `Agent "${key}" tool "${tool}" missing mcp__ prefix`,
          );
        }
      });
    }
  });
});
