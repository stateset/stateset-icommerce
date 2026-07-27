/**
 * Integration test for tool-backed command action coverage.
 *
 * Dynamically enumerates every command module that exports a `toolActionMap`
 * and pairs it with the same-named tools module, then asserts every tool maps
 * to exactly one action. Dynamic on purpose: the earlier hardcoded list only
 * covered the eight original a2a/x402 modules, so the twenty-six tool-backed
 * modules added later would have shipped without this invariant.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const commandsDir = path.resolve(__dirname, '../../src/commands');
const toolsDir = path.resolve(__dirname, '../../src/tools');

async function collectToolBackedModules() {
  const modules = [];
  for (const entry of fs.readdirSync(commandsDir).sort()) {
    if (!entry.endsWith('.js') || entry === 'index.js') continue;
    const name = entry.replace(/\.js$/, '');
    const command = await import(pathToFileURL(path.join(commandsDir, entry)).href);
    if (!Array.isArray(command.toolActionMap)) continue;

    const toolsPath = path.join(toolsDir, entry);
    assert.ok(
      fs.existsSync(toolsPath),
      `${name}: exports toolActionMap but has no matching tools module`,
    );
    const toolsModule = await import(pathToFileURL(toolsPath).href);
    const tools = Object.values(toolsModule).find(
      (value) => Array.isArray(value) && value.every((tool) => tool?.name && tool?.handler),
    );
    assert.ok(tools, `${name}: no tool array export found in tools module`);
    modules.push([name, tools, command.toolActionMap]);
  }
  return modules;
}

const TOOL_BACKED_MODULES = await collectToolBackedModules();

describe('tool-backed command coverage', () => {
  it('discovers a sensible number of tool-backed modules', () => {
    // 8 original (a2a*, agent-cards, agent-runtime, x402) + 26 generated.
    assert.ok(
      TOOL_BACKED_MODULES.length >= 34,
      `expected >= 34 tool-backed modules, found ${TOOL_BACKED_MODULES.length}`,
    );
  });

  for (const [moduleName, tools, toolActionMap] of TOOL_BACKED_MODULES) {
    it(`${moduleName} should map every tool exactly once`, () => {
      const toolNames = tools.map((tool) => tool.name).sort();
      const mappedToolNames = toolActionMap.map((entry) => entry.tool).sort();

      assert.deepStrictEqual(mappedToolNames, toolNames);
      assert.strictEqual(new Set(mappedToolNames).size, mappedToolNames.length);
      assert.strictEqual(
        new Set(toolActionMap.map((entry) => entry.action)).size,
        toolActionMap.length,
      );
    });
  }
});
