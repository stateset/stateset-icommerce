/**
 * Integration test for top-level API command coverage.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { commands } from '../../src/commands/index.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const toolsDir = path.resolve(__dirname, '../../src/tools');
const commandsDir = path.resolve(__dirname, '../../src/commands');

function listModules(dirPath, exclude = []) {
  const excluded = new Set(exclude);
  return fs
    .readdirSync(dirPath)
    .filter((entry) => entry.endsWith('.js') && !excluded.has(entry))
    .map((entry) => entry.replace(/\.js$/, ''))
    .sort();
}

describe('api command coverage', () => {
  it('should cover every top-level tool module with a registered command module', () => {
    const toolModules = listModules(toolsDir, ['index.js', 'domain-registry.js']);
    const commandModules = Object.keys(commands).sort();
    assert.deepStrictEqual(commandModules, toolModules);
  });

  it('should have every command file registered in the command registry', () => {
    const commandModulesOnDisk = listModules(commandsDir, ['index.js']);
    const commandModules = Object.keys(commands).sort();
    assert.deepStrictEqual(commandModules, commandModulesOnDisk);
  });
});
