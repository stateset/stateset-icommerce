/**
 * Integration test for tool-backed command dispatch.
 *
 * Executes every tool-backed command action against stubbed tool handlers to
 * ensure runtime dispatch stays aligned with the declared tool mapping.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { agentCardTools } from '../../src/tools/agent-cards.js';
import { agentRuntimeTools } from '../../src/tools/agent-runtime.js';
import { a2aPlatformTools } from '../../src/tools/a2a-platform.js';
import { a2aObservabilityTools } from '../../src/tools/a2a-observability.js';
import { a2aIntelligenceTools } from '../../src/tools/a2a-intelligence.js';
import { a2aAutomationTools } from '../../src/tools/a2a-automation.js';
import { a2aTools } from '../../src/tools/a2a.js';
import { x402Tools } from '../../src/tools/x402.js';
import * as agentCardsCommand from '../../src/commands/agent-cards.js';
import * as agentRuntimeCommand from '../../src/commands/agent-runtime.js';
import * as a2aPlatformCommand from '../../src/commands/a2a-platform.js';
import * as a2aObservabilityCommand from '../../src/commands/a2a-observability.js';
import * as a2aIntelligenceCommand from '../../src/commands/a2a-intelligence.js';
import * as a2aAutomationCommand from '../../src/commands/a2a-automation.js';
import * as a2aCommand from '../../src/commands/a2a.js';
import * as x402Command from '../../src/commands/x402.js';

const TOOL_BACKED_MODULES = [
  ['agent-cards', agentCardsCommand, agentCardTools, agentCardsCommand.toolActionMap],
  ['agent-runtime', agentRuntimeCommand, agentRuntimeTools, agentRuntimeCommand.toolActionMap],
  ['a2a-platform', a2aPlatformCommand, a2aPlatformTools, a2aPlatformCommand.toolActionMap],
  ['a2a-observability', a2aObservabilityCommand, a2aObservabilityTools, a2aObservabilityCommand.toolActionMap],
  ['a2a-intelligence', a2aIntelligenceCommand, a2aIntelligenceTools, a2aIntelligenceCommand.toolActionMap],
  ['a2a-automation', a2aAutomationCommand, a2aAutomationTools, a2aAutomationCommand.toolActionMap],
  ['a2a', a2aCommand, a2aTools, a2aCommand.toolActionMap],
  ['x402', x402Command, x402Tools, x402Command.toolActionMap],
];

function extractArgName(spec) {
  const match = String(spec).match(/^[<[](.+)[>\]]$/);
  return match ? match[1] : String(spec);
}

function sampleJsonValue(key) {
  if (key.includes('payments')) return '[{"id":"payment-1"}]';
  if (key.includes('requests')) return '[{"id":"request-1"}]';
  if (key.includes('targets')) return '["0xagent1","0xagent2"]';
  if (
    key.includes('volumebreaks') ||
    key.includes('reputationtiers') ||
    key.includes('peakhours') ||
    key.includes('loyaltytiers')
  ) {
    return '[]';
  }
  return '{"id":"sample"}';
}

function sampleArgValue(spec) {
  const argName = extractArgName(spec);
  const key = argName.toLowerCase();

  if (key.endsWith('json')) return sampleJsonValue(key);
  if (key.endsWith('csv')) return 'alpha,beta';
  if (
    ['active', 'enabled', 'simulate', 'redact', 'unreadonly', 'includecompleted', 'refreshonchain'].includes(
      key,
    )
  ) {
    return 'true';
  }
  if (
    [
      'limit',
      'days',
      'trenddays',
      'lookbackdays',
      'intervalms',
      'timeoutms',
      'concurrency',
      'maxexecutions',
      'responsetimems',
      'repeatinterval',
    ].includes(key) ||
    key.endsWith('limit')
  ) {
    return '5';
  }
  if (['amount', 'reward', 'monthlybudget'].includes(key) || key.includes('budget')) {
    return '12.5';
  }
  if (['executeat', 'deadline', 'since', 'until'].includes(key)) return '2026-01-01T00:00:00Z';
  if (key === 'network' || key.endsWith('network')) return 'base';
  if (key === 'asset') return 'usdc';
  if (key === 'trustlevel') return 'verified';
  if (key === 'chainid') return 'base';
  if (key === 'tokensymbol') return 'USDC';
  if (key === 'status') return 'active';
  if (key === 'type') return 'quote';
  if (key === 'actiontype') return 'rebalance';
  if (key === 'tasktype') return 'quote';
  if (key === 'interactiontype') return 'payment';
  if (key === 'sortby') return 'score';
  if (key === 'joinstrategy') return 'all';
  if (key === 'strategy') return 'balanced';
  if (key === 'category') return 'marketplace';
  if (key === 'pricingmodel') return 'fixed';
  if (key === 'priority') return 'high';
  if (key === 'outcome') return 'success';
  if (key === 'to') return '0xreceiver';
  if (key.includes('address')) return '0xabc123';
  if (key.includes('name')) return 'runtime-1';
  if (key.endsWith('id')) return `${argName}-1`;
  if (key === 'secret') return 'secret-key';
  if (key === 'rawbody') return '{"ok":true}';
  if (key === 'signatureheader') return 'sig-header';
  if (key === 'timestampheader') return 'ts-header';

  return `sample-${argName}`;
}

function buildArgs(actionArgs) {
  return (actionArgs || []).map(sampleArgValue);
}

function createContext() {
  return {
    commerce: { stub: true },
    output: null,
    jsonOutput: false,
  };
}

describe('tool-backed command dispatch', () => {
  for (const [moduleName, commandModule, tools, toolActionMap] of TOOL_BACKED_MODULES) {
    it(`${moduleName} should dispatch every mapped action to the expected tool`, async () => {
      const calls = [];
      const originals = tools.map((tool) => [tool, tool.handler]);

      try {
        for (const tool of tools) {
          tool.handler = async (toolContext) => {
            calls.push({ toolName: tool.name, toolContext });
            return {
              success: true,
              toolName: tool.name,
              params: toolContext.params,
              agentAddress: toolContext.agentConfig?.walletAddress ?? null,
            };
          };
        }

        const context = createContext();

        for (const { action, tool } of toolActionMap) {
          const actionArgs = commandModule.metadata.actions[action]?.args ?? [];
          const args = buildArgs(actionArgs);
          const previousCallCount = calls.length;
          const result = await commandModule.execute(action, args, context);

          assert.strictEqual(
            calls.length,
            previousCallCount + 1,
            `${moduleName} ${action} should invoke exactly one tool`,
          );

          const call = calls.at(-1);
          assert.ok(call, `${moduleName} ${action} should record a tool invocation`);
          assert.strictEqual(call.toolName, tool);
          assert.strictEqual(call.toolContext.commerce, context.commerce);
          assert.strictEqual(call.toolContext.allowApply, true);
          assert.strictEqual(result?.result?.toolName, tool);

          const agentAddressIndex = actionArgs.findIndex(
            (spec) => extractArgName(spec) === 'agentAddress',
          );

          if (agentAddressIndex !== -1) {
            const expectedAgentAddress = args[agentAddressIndex];
            const observedAgentAddress =
              call.toolContext.agentConfig?.walletAddress ?? call.toolContext.params?.agentAddress;
            assert.strictEqual(observedAgentAddress, expectedAgentAddress);
          }
        }
      } finally {
        for (const [tool, originalHandler] of originals) {
          tool.handler = originalHandler;
        }
      }
    });
  }
});
