#!/usr/bin/env node

import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createToolRegistry } from '../../cli/src/tools/index.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '../..');
const jsonOutputPath = path.join(rootDir, 'artifacts/compatibility/mcp-tool-inventory.json');
const markdownOutputPath = path.join(rootDir, 'docs/src/appendix/mcp-tool-inventory.md');
const checkMode = process.argv.includes('--check');

function compareStrings(left, right) {
  return left.replace(/\r\n/g, '\n') === right.replace(/\r\n/g, '\n');
}

function renderMarkdownTable(headers, rows) {
  const headerRow = `| ${headers.join(' | ')} |`;
  const dividerRow = `| ${headers.map(() => '---').join(' | ')} |`;
  const bodyRows = rows.map((row) => `| ${row.join(' | ')} |`);
  return [headerRow, dividerRow, ...bodyRows].join('\n');
}

function renderMarkdownInventory(inventory) {
  const summaryRows = [
    ['Total tools', String(inventory.totalTools)],
    ['Loaded modules', String(inventory.moduleCount)],
    ['Read tools', String(inventory.permissionCounts.read ?? 0)],
    ['Write tools', String(inventory.permissionCounts.write ?? 0)],
    ['Delete tools', String(inventory.permissionCounts.delete ?? 0)],
    ['Admin tools', String(inventory.permissionCounts.admin ?? 0)],
  ];

  const moduleRows = inventory.modules.map((entry) => [entry.name, String(entry.count)]);
  const permissionRows = inventory.permissions.map((entry) => [entry.name, String(entry.count)]);
  const toolRows = inventory.tools.map((tool) => [
    `\`${tool.name}\``,
    `\`${tool.module}\``,
    `\`${tool.permission}\``,
  ]);

  return `# MCP Tool Inventory

This page is generated from the live CLI registry in \`cli/src/tools/index.js\`.
Do not edit it by hand. Regenerate it with:

\`\`\`bash
node ./scripts/ci/generate_mcp_inventory.mjs
\`\`\`

Machine-readable output lives at \`artifacts/compatibility/mcp-tool-inventory.json\`.

## Summary

${renderMarkdownTable(['Metric', 'Value'], summaryRows)}

## Module Counts

${renderMarkdownTable(['Module', 'Tools'], moduleRows)}

## Permission Counts

${renderMarkdownTable(['Permission', 'Tools'], permissionRows)}

## Tool Registry

${renderMarkdownTable(['Tool', 'Module', 'Permission'], toolRows)}
`;
}

async function buildInventory() {
  const registry = createToolRegistry();
  await registry.loadAll();

  const tools = registry
    .getAll()
    .map((tool) => ({
      name: tool.name,
      description: tool.description,
      module: tool.category,
      permission: tool.permission ?? 'unspecified',
    }))
    .sort((left, right) => left.name.localeCompare(right.name));

  const moduleCounts = new Map();
  const permissionCounts = new Map();

  for (const tool of tools) {
    moduleCounts.set(tool.module, (moduleCounts.get(tool.module) ?? 0) + 1);
    permissionCounts.set(tool.permission, (permissionCounts.get(tool.permission) ?? 0) + 1);
  }

  const modules = [...moduleCounts.entries()]
    .sort((left, right) => left[0].localeCompare(right[0]))
    .map(([name, count]) => ({ name, count }));

  const permissions = [...permissionCounts.entries()]
    .sort((left, right) => left[0].localeCompare(right[0]))
    .map(([name, count]) => ({ name, count }));

  return {
    source: 'cli/src/tools/index.js',
    totalTools: tools.length,
    moduleCount: modules.length,
    modules,
    permissions,
    permissionCounts: Object.fromEntries(permissions.map((entry) => [entry.name, entry.count])),
    tools,
  };
}

async function verifyOutput(filePath, expectedContent) {
  const relativePath = path.relative(rootDir, filePath);

  try {
    const actualContent = await readFile(filePath, 'utf8');
    if (!compareStrings(actualContent, expectedContent)) {
      console.error(
        `::error file=${relativePath}::Generated MCP inventory is out of date. Run 'node ./scripts/ci/generate_mcp_inventory.mjs'.`,
      );
      return false;
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : 'unknown error';
    console.error(
      `::error file=${relativePath}::Unable to read generated MCP inventory output (${message}). Run 'node ./scripts/ci/generate_mcp_inventory.mjs'.`,
    );
    return false;
  }

  return true;
}

async function main() {
  const inventory = await buildInventory();
  const jsonContent = `${JSON.stringify(inventory, null, 2)}\n`;
  const markdownContent = renderMarkdownInventory(inventory);

  if (checkMode) {
    const ok = await Promise.all([
      verifyOutput(jsonOutputPath, jsonContent),
      verifyOutput(markdownOutputPath, markdownContent),
    ]);

    if (!ok.every(Boolean)) {
      process.exit(1);
    }

    console.log(
      `MCP inventory is up to date (${inventory.totalTools} tools across ${inventory.moduleCount} modules).`,
    );
    return;
  }

  await mkdir(path.dirname(jsonOutputPath), { recursive: true });
  await mkdir(path.dirname(markdownOutputPath), { recursive: true });
  await writeFile(jsonOutputPath, jsonContent, 'utf8');
  await writeFile(markdownOutputPath, markdownContent, 'utf8');

  console.log(
    `Generated MCP inventory (${inventory.totalTools} tools across ${inventory.moduleCount} modules).`,
  );
}

await main();
