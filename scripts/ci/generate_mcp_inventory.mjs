#!/usr/bin/env node

import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { getStaticMcpToolDefinitions } from '../../cli/src/mcp-server.js';

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
    ['Policy domains', String(inventory.domainCount)],
    ['Read tools', String(inventory.permissionCounts.read ?? 0)],
    ['Write tools', String(inventory.permissionCounts.write ?? 0)],
    ['Delete tools', String(inventory.permissionCounts.delete ?? 0)],
    ['Admin tools', String(inventory.permissionCounts.admin ?? 0)],
    ['Unknown permission', String(inventory.permissionCounts.unknown ?? 0)],
  ];

  const domainRows = inventory.domains.map((entry) => [entry.name, String(entry.count)]);
  const permissionRows = inventory.permissions.map((entry) => [entry.name, String(entry.count)]);
  const toolRows = inventory.tools.map((tool) => [
    `\`${tool.name}\``,
    `\`${tool.policyDomain}\``,
    `\`${tool.permission}\``,
  ]);

  return `# MCP Tool Inventory

This page is generated from the live MCP server export in \`cli/src/mcp-server.js\`.
Do not edit it by hand. Regenerate it with:

\`\`\`bash
node ./scripts/ci/generate_mcp_inventory.mjs
\`\`\`

Machine-readable output lives at \`artifacts/compatibility/mcp-tool-inventory.json\`.

## Summary

${renderMarkdownTable(['Metric', 'Value'], summaryRows)}

## Policy Domain Counts

${renderMarkdownTable(['Policy domain', 'Tools'], domainRows)}

## Permission Counts

${renderMarkdownTable(['Permission', 'Tools'], permissionRows)}

## Tool Registry

${renderMarkdownTable(['Tool', 'Policy domain', 'Permission'], toolRows)}
`;
}

async function buildInventory() {
  const tools = getStaticMcpToolDefinitions()
    .map((tool) => ({
      name: tool.name,
      description: tool.description,
      policyDomain: tool.policyDomain ?? 'commerce',
      permission: tool.permission ?? 'unspecified',
    }))
    .sort((left, right) => left.name.localeCompare(right.name));

  const domainCounts = new Map();
  const permissionCounts = new Map();

  for (const tool of tools) {
    domainCounts.set(tool.policyDomain, (domainCounts.get(tool.policyDomain) ?? 0) + 1);
    permissionCounts.set(tool.permission, (permissionCounts.get(tool.permission) ?? 0) + 1);
  }

  const domains = [...domainCounts.entries()]
    .sort((left, right) => left[0].localeCompare(right[0]))
    .map(([name, count]) => ({ name, count }));

  const permissions = [...permissionCounts.entries()]
    .sort((left, right) => left[0].localeCompare(right[0]))
    .map(([name, count]) => ({ name, count }));

  return {
    source: 'cli/src/mcp-server.js',
    totalTools: tools.length,
    domainCount: domains.length,
    domains,
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
      `MCP inventory is up to date (${inventory.totalTools} tools across ${inventory.domainCount} policy domains).`,
    );
    return;
  }

  await mkdir(path.dirname(jsonOutputPath), { recursive: true });
  await mkdir(path.dirname(markdownOutputPath), { recursive: true });
  await writeFile(jsonOutputPath, jsonContent, 'utf8');
  await writeFile(markdownOutputPath, markdownContent, 'utf8');

  console.log(
    `Generated MCP inventory (${inventory.totalTools} tools across ${inventory.domainCount} policy domains).`,
  );
}

await main();
