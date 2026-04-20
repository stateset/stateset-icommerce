/**
 * Integration test for tool-backed command action coverage.
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
import { toolActionMap as agentCardsToolActionMap } from '../../src/commands/agent-cards.js';
import { toolActionMap as agentRuntimeToolActionMap } from '../../src/commands/agent-runtime.js';
import { toolActionMap as a2aPlatformToolActionMap } from '../../src/commands/a2a-platform.js';
import { toolActionMap as a2aObservabilityToolActionMap } from '../../src/commands/a2a-observability.js';
import { toolActionMap as a2aIntelligenceToolActionMap } from '../../src/commands/a2a-intelligence.js';
import { toolActionMap as a2aAutomationToolActionMap } from '../../src/commands/a2a-automation.js';
import { toolActionMap as a2aToolActionMap } from '../../src/commands/a2a.js';
import { toolActionMap as x402ToolActionMap } from '../../src/commands/x402.js';

const TOOL_BACKED_MODULES = [
  ['agent-cards', agentCardTools, agentCardsToolActionMap],
  ['agent-runtime', agentRuntimeTools, agentRuntimeToolActionMap],
  ['a2a-platform', a2aPlatformTools, a2aPlatformToolActionMap],
  ['a2a-observability', a2aObservabilityTools, a2aObservabilityToolActionMap],
  ['a2a-intelligence', a2aIntelligenceTools, a2aIntelligenceToolActionMap],
  ['a2a-automation', a2aAutomationTools, a2aAutomationToolActionMap],
  ['a2a', a2aTools, a2aToolActionMap],
  ['x402', x402Tools, x402ToolActionMap],
];

describe('tool-backed command coverage', () => {
  for (const [moduleName, tools, toolActionMap] of TOOL_BACKED_MODULES) {
    it(`${moduleName} should map every tool exactly once`, () => {
      const toolNames = tools.map((tool) => tool.name).sort();
      const mappedToolNames = toolActionMap.map((entry) => entry.tool).sort();

      assert.deepStrictEqual(mappedToolNames, toolNames);
      assert.strictEqual(new Set(mappedToolNames).size, mappedToolNames.length);
      assert.strictEqual(new Set(toolActionMap.map((entry) => entry.action)).size, toolActionMap.length);
    });
  }
});
