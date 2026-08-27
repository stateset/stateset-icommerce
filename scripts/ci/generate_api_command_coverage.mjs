#!/usr/bin/env node

import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { commands, RESOURCE_ALIASES } from '../../cli/src/commands/index.js';
import { agentCardTools } from '../../cli/src/tools/agent-cards.js';
import { agentRuntimeTools } from '../../cli/src/tools/agent-runtime.js';
import { a2aPlatformTools } from '../../cli/src/tools/a2a-platform.js';
import { a2aObservabilityTools } from '../../cli/src/tools/a2a-observability.js';
import { a2aIntelligenceTools } from '../../cli/src/tools/a2a-intelligence.js';
import { a2aAutomationTools } from '../../cli/src/tools/a2a-automation.js';
import { a2aTools } from '../../cli/src/tools/a2a.js';
import { x402Tools } from '../../cli/src/tools/x402.js';
import { toolActionMap as agentCardsToolActionMap } from '../../cli/src/commands/agent-cards.js';
import { toolActionMap as agentRuntimeToolActionMap } from '../../cli/src/commands/agent-runtime.js';
import { toolActionMap as a2aPlatformToolActionMap } from '../../cli/src/commands/a2a-platform.js';
import { toolActionMap as a2aObservabilityToolActionMap } from '../../cli/src/commands/a2a-observability.js';
import { toolActionMap as a2aIntelligenceToolActionMap } from '../../cli/src/commands/a2a-intelligence.js';
import { toolActionMap as a2aAutomationToolActionMap } from '../../cli/src/commands/a2a-automation.js';
import { toolActionMap as a2aToolActionMap } from '../../cli/src/commands/a2a.js';
import { toolActionMap as x402ToolActionMap } from '../../cli/src/commands/x402.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '../..');
const toolsDir = path.join(rootDir, 'cli/src/tools');
const commandsDir = path.join(rootDir, 'cli/src/commands');
const jsonOutputPath = path.join(rootDir, 'artifacts/compatibility/api-command-coverage.json');
const markdownOutputPath = path.join(rootDir, 'docs/src/appendix/api-command-coverage.md');
const checkMode = process.argv.includes('--check');

const TOOL_BACKED_COMMANDS = new Map([
  ['agent-cards', { tools: agentCardTools, actionMap: agentCardsToolActionMap }],
  ['agent-runtime', { tools: agentRuntimeTools, actionMap: agentRuntimeToolActionMap }],
  ['a2a-platform', { tools: a2aPlatformTools, actionMap: a2aPlatformToolActionMap }],
  ['a2a-observability', { tools: a2aObservabilityTools, actionMap: a2aObservabilityToolActionMap }],
  ['a2a-intelligence', { tools: a2aIntelligenceTools, actionMap: a2aIntelligenceToolActionMap }],
  ['a2a-automation', { tools: a2aAutomationTools, actionMap: a2aAutomationToolActionMap }],
  ['a2a', { tools: a2aTools, actionMap: a2aToolActionMap }],
  ['x402', { tools: x402Tools, actionMap: x402ToolActionMap }],
]);

function compareStrings(left, right) {
  return left.replace(/\r\n/g, '\n') === right.replace(/\r\n/g, '\n');
}

function renderMarkdownTable(headers, rows) {
  const headerRow = `| ${headers.join(' | ')} |`;
  const dividerRow = `| ${headers.map(() => '---').join(' | ')} |`;
  const bodyRows = rows.map((row) => `| ${row.join(' | ')} |`);
  return [headerRow, dividerRow, ...bodyRows].join('\n');
}

async function listModuleNames(dirPath, { exclude = [] } = {}) {
  const excluded = new Set(exclude);
  return (await readdir(dirPath))
    .filter((entry) => entry.endsWith('.js') && !excluded.has(entry))
    .map((entry) => entry.replace(/\.js$/, ''))
    .sort((left, right) => left.localeCompare(right));
}

async function detectCoverageStyle(moduleName) {
  const commandSource = await readFile(path.join(commandsDir, `${moduleName}.js`), 'utf8');
  return commandSource.includes(`../tools/${moduleName}.js`) ? 'tool-backed' : 'custom';
}

async function buildInventory() {
  const toolModules = await listModuleNames(toolsDir, {
    exclude: ['index.js', 'domain-registry.js'],
  });
  const commandModulesOnDisk = await listModuleNames(commandsDir, { exclude: ['index.js'] });
  const commandRegistryModules = Object.keys(commands).sort((left, right) =>
    left.localeCompare(right),
  );

  const uncoveredToolModules = toolModules.filter((name) => !commandRegistryModules.includes(name));
  const commandOnlyModules = commandRegistryModules.filter((name) => !toolModules.includes(name));
  const registryMismatches = commandModulesOnDisk.filter(
    (name) => !commandRegistryModules.includes(name),
  );
  const missingOnDisk = commandRegistryModules.filter(
    (name) => !commandModulesOnDisk.includes(name),
  );

  const coverageByModule = await Promise.all(
    commandRegistryModules.map(async (name) => {
      const toolBacked = TOOL_BACKED_COMMANDS.get(name);
      const mappedToolNames = new Set((toolBacked?.actionMap || []).map((entry) => entry.tool));
      const actualToolNames = new Set((toolBacked?.tools || []).map((entry) => entry.name));
      const uncoveredToolNames = [...actualToolNames]
        .filter((tool) => !mappedToolNames.has(tool))
        .sort();
      const unknownMappedToolNames = [...mappedToolNames]
        .filter((tool) => !actualToolNames.has(tool))
        .sort();
      return {
        module: name,
        toolModulePresent: toolModules.includes(name),
        commandFilePresent: commandModulesOnDisk.includes(name),
        actionCount: Object.keys(commands[name]?.metadata?.actions ?? {}).length,
        aliasCount: Object.entries(RESOURCE_ALIASES).filter(([, target]) => target === name).length,
        coverageStyle: toolModules.includes(name)
          ? await detectCoverageStyle(name)
          : 'command-only',
        toolBacked: Boolean(toolBacked),
        toolCount: actualToolNames.size,
        mappedToolCount: mappedToolNames.size,
        uncoveredToolNames,
        unknownMappedToolNames,
      };
    }),
  );

  if (coverageByModule.some((entry) => !entry.module || !entry.coverageStyle)) {
    throw new Error('Coverage generator produced an incomplete module entry');
  }

  const toolBackedCoverage = coverageByModule.filter((entry) => entry.toolBacked);
  const uncoveredToolBackedActions = toolBackedCoverage.reduce(
    (count, entry) => count + entry.uncoveredToolNames.length + entry.unknownMappedToolNames.length,
    0,
  );

  return {
    source: {
      toolsDir: 'cli/src/tools',
      commandsDir: 'cli/src/commands',
      commandRegistry: 'cli/src/commands/index.js',
    },
    summary: {
      totalToolModules: toolModules.length,
      totalCommandModulesOnDisk: commandModulesOnDisk.length,
      totalCommandRegistryModules: commandRegistryModules.length,
      totalToolBackedCommandModules: toolBackedCoverage.length,
      uncoveredToolModules: uncoveredToolModules.length,
      uncoveredToolBackedActions,
      commandOnlyModules: commandOnlyModules.length,
      registryMismatches: registryMismatches.length + missingOnDisk.length,
      fullyCovered:
        uncoveredToolModules.length === 0 &&
        uncoveredToolBackedActions === 0 &&
        commandOnlyModules.length === 0 &&
        registryMismatches.length === 0 &&
        missingOnDisk.length === 0,
    },
    uncoveredToolModules,
    commandOnlyModules,
    registryMismatches: {
      onDiskNotRegistered: registryMismatches,
      registeredMissingOnDisk: missingOnDisk,
    },
    coverageByModule,
  };
}

function renderMarkdownInventory(inventory) {
  const summaryRows = [
    ['Tool modules', String(inventory.summary.totalToolModules)],
    ['Command modules on disk', String(inventory.summary.totalCommandModulesOnDisk)],
    ['Command modules in registry', String(inventory.summary.totalCommandRegistryModules)],
    ['Tool-backed command modules', String(inventory.summary.totalToolBackedCommandModules)],
    ['Uncovered tool modules', String(inventory.summary.uncoveredToolModules)],
    ['Uncovered tool-backed actions', String(inventory.summary.uncoveredToolBackedActions)],
    ['Command-only modules', String(inventory.summary.commandOnlyModules)],
    ['Registry mismatches', String(inventory.summary.registryMismatches)],
    ['Fully covered', inventory.summary.fullyCovered ? 'yes' : 'no'],
  ];

  const moduleRows = inventory.coverageByModule.map((entry) => [
    `\`${entry.module}\``,
    entry.coverageStyle,
    String(entry.actionCount),
    String(entry.aliasCount),
    entry.toolBacked ? `${entry.mappedToolCount}/${entry.toolCount}` : '-',
  ]);

  const uncoveredRows =
    inventory.uncoveredToolModules.length === 0
      ? [['None', '-', '-', '-']]
      : inventory.uncoveredToolModules.map((entry) => [
          `\`${entry}\``,
          'missing command coverage',
          '-',
          '-',
        ]);

  const uncoveredToolBackedRows = inventory.coverageByModule
    .filter(
      (entry) =>
        entry.toolBacked &&
        (entry.uncoveredToolNames.length > 0 || entry.unknownMappedToolNames.length > 0),
    )
    .flatMap((entry) => {
      const rows = [];
      for (const tool of entry.uncoveredToolNames) {
        rows.push([`\`${entry.module}\``, `\`${tool}\``, 'missing command action']);
      }
      for (const tool of entry.unknownMappedToolNames) {
        rows.push([`\`${entry.module}\``, `\`${tool}\``, 'mapped by command but missing tool']);
      }
      return rows;
    });
  const uncoveredToolBackedTableRows =
    uncoveredToolBackedRows.length === 0 ? [['None', '-', '-']] : uncoveredToolBackedRows;

  return `# API Command Coverage

This page is generated from the live tool modules in \`cli/src/tools\` and the CLI command registry in
\`cli/src/commands/index.js\`. Do not edit it by hand. Regenerate it with:

\`\`\`bash
node ./scripts/ci/generate_api_command_coverage.mjs
\`\`\`

Machine-readable output lives at \`artifacts/compatibility/api-command-coverage.json\`.

## Summary

${renderMarkdownTable(['Metric', 'Value'], summaryRows)}

## Module Coverage

${renderMarkdownTable(['Module', 'Coverage style', 'Actions', 'Aliases', 'Tool-backed action coverage'], moduleRows)}

## Uncovered Tool Modules

${renderMarkdownTable(['Module', 'Status', 'Actions', 'Aliases'], uncoveredRows)}

## Uncovered Tool-Backed Actions

${renderMarkdownTable(['Module', 'Tool', 'Status'], uncoveredToolBackedTableRows)}
`;
}

async function verifyOutput(filePath, expectedContent, instruction) {
  const relativePath = path.relative(rootDir, filePath);

  try {
    const actualContent = await readFile(filePath, 'utf8');
    if (!compareStrings(actualContent, expectedContent)) {
      console.error(`::error file=${relativePath}::${instruction}`);
      return false;
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : 'unknown error';
    console.error(
      `::error file=${relativePath}::Unable to read generated coverage output (${message}). ${instruction}`,
    );
    return false;
  }

  return true;
}

async function main() {
  const inventory = await buildInventory();
  const jsonContent = `${JSON.stringify(inventory, null, 2)}\n`;
  const markdownContent = renderMarkdownInventory(inventory);
  const instruction = "Run 'node ./scripts/ci/generate_api_command_coverage.mjs'.";

  if (checkMode) {
    const ok = await Promise.all([
      verifyOutput(jsonOutputPath, jsonContent, instruction),
      verifyOutput(markdownOutputPath, markdownContent, instruction),
    ]);

    if (!ok.every(Boolean)) process.exit(1);

    console.log(
      `API command coverage is up to date (${inventory.summary.totalToolModules} tool modules, fullyCovered=${inventory.summary.fullyCovered}).`,
    );
    return;
  }

  await mkdir(path.dirname(jsonOutputPath), { recursive: true });
  await mkdir(path.dirname(markdownOutputPath), { recursive: true });
  await writeFile(jsonOutputPath, jsonContent, 'utf8');
  await writeFile(markdownOutputPath, markdownContent, 'utf8');

  console.log(
    `Generated API command coverage (${inventory.summary.totalToolModules} tool modules, fullyCovered=${inventory.summary.fullyCovered}).`,
  );
}

await main();
