#!/usr/bin/env node

import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { getBuiltinHttpRouteDefinitions } from '../../cli/src/channels/http-gateway.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '../..');
const jsonOutputPath = path.join(rootDir, 'artifacts/compatibility/http-gateway-inventory.json');
const markdownOutputPath = path.join(rootDir, 'docs/src/appendix/http-gateway-inventory.md');
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

function buildInventory() {
  const routes = getBuiltinHttpRouteDefinitions().sort(
    (left, right) =>
      left.path.localeCompare(right.path) || left.method.localeCompare(right.method),
  );

  const tagCounts = new Map();
  const permissionCounts = new Map();
  for (const route of routes) {
    for (const tag of route.tags || []) {
      tagCounts.set(tag, (tagCounts.get(tag) ?? 0) + 1);
    }
    permissionCounts.set(route.level, (permissionCounts.get(route.level) ?? 0) + 1);
  }

  return {
    source: 'cli/src/channels/http-gateway.js',
    totalRoutes: routes.length,
    tagCount: tagCounts.size,
    permissionCount: permissionCounts.size,
    permissions: [...permissionCounts.entries()]
      .sort((left, right) => left[0].localeCompare(right[0]))
      .map(([name, count]) => ({ name, count })),
    tags: [...tagCounts.entries()]
      .sort((left, right) => left[0].localeCompare(right[0]))
      .map(([name, count]) => ({ name, count })),
    routes,
  };
}

function renderMarkdownInventory(inventory) {
  const summaryRows = [
    ['Total built-in routes', String(inventory.totalRoutes)],
    ['Permission levels', String(inventory.permissionCount)],
    ['Tags', String(inventory.tagCount)],
  ];

  const permissionRows = inventory.permissions.map((entry) => [entry.name, String(entry.count)]);
  const tagRows = inventory.tags.map((entry) => [entry.name, String(entry.count)]);
  const routeRows = inventory.routes.map((route) => [
    `\`${route.method}\``,
    `\`${route.openapiPath}\``,
    `\`${route.level}\``,
    (route.tags || []).map((tag) => `\`${tag}\``).join(', ') || '—',
    route.summary,
  ]);

  return `# HTTP Gateway Inventory

This page is generated from the built-in HTTP gateway route registry in \`cli/src/channels/http-gateway.js\`.
Do not edit it by hand. Regenerate it with:

\`\`\`bash
node ./scripts/ci/generate_http_gateway_inventory.mjs
\`\`\`

Machine-readable output lives at \`artifacts/compatibility/http-gateway-inventory.json\`.

## Summary

${renderMarkdownTable(['Metric', 'Value'], summaryRows)}

## Permission Counts

${renderMarkdownTable(['Permission', 'Routes'], permissionRows)}

## Tag Counts

${renderMarkdownTable(['Tag', 'Routes'], tagRows)}

## Built-in Routes

${renderMarkdownTable(['Method', 'OpenAPI path', 'Permission', 'Tags', 'Summary'], routeRows)}
`;
}

async function verifyOutput(filePath, expectedContent) {
  const relativePath = path.relative(rootDir, filePath);

  try {
    const actualContent = await readFile(filePath, 'utf8');
    if (!compareStrings(actualContent, expectedContent)) {
      console.error(
        `::error file=${relativePath}::Generated HTTP gateway inventory is out of date. Run 'node ./scripts/ci/generate_http_gateway_inventory.mjs'.`,
      );
      return false;
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : 'unknown error';
    console.error(
      `::error file=${relativePath}::Unable to read generated HTTP gateway inventory output (${message}). Run 'node ./scripts/ci/generate_http_gateway_inventory.mjs'.`,
    );
    return false;
  }

  return true;
}

async function main() {
  const inventory = buildInventory();
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

    console.log(`HTTP gateway inventory is up to date (${inventory.totalRoutes} built-in routes).`);
    return;
  }

  await mkdir(path.dirname(jsonOutputPath), { recursive: true });
  await mkdir(path.dirname(markdownOutputPath), { recursive: true });
  await writeFile(jsonOutputPath, jsonContent, 'utf8');
  await writeFile(markdownOutputPath, markdownContent, 'utf8');

  console.log(`Generated HTTP gateway inventory (${inventory.totalRoutes} built-in routes).`);
}

await main();
