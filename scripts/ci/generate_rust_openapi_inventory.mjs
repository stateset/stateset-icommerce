#!/usr/bin/env node

import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '../..');
const jsonOutputPath = path.join(rootDir, 'artifacts/compatibility/rust-openapi-inventory.json');
const markdownOutputPath = path.join(rootDir, 'docs/src/appendix/rust-openapi-inventory.md');
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

function runOpenapiExporter() {
  const run = spawnSync('cargo', ['run', '-q', '-p', 'stateset-http', '--example', 'export_openapi'], {
    cwd: rootDir,
    encoding: 'utf8',
  });

  if (run.status !== 0) {
    throw new Error(run.stderr || run.stdout || 'failed to export Rust OpenAPI spec');
  }

  try {
    return JSON.parse(run.stdout);
  } catch (error) {
    throw new Error(`failed to parse exported Rust OpenAPI spec: ${error}\n${run.stdout}`);
  }
}

function buildInventory(spec) {
  const paths = spec.paths || {};
  const pathEntries = Object.entries(paths);
  const schemas = spec.components?.schemas || {};
  const tags = Array.isArray(spec.tags) ? spec.tags : [];
  const operations = [];
  const methodCounts = new Map();
  const tagCounts = new Map();

  for (const [pathKey, pathItem] of pathEntries) {
    for (const [method, operation] of Object.entries(pathItem || {})) {
      if (!['get', 'post', 'put', 'patch', 'delete', 'head', 'options'].includes(method)) {
        continue;
      }
      methodCounts.set(method.toUpperCase(), (methodCounts.get(method.toUpperCase()) ?? 0) + 1);
      const operationTags = Array.isArray(operation?.tags) ? operation.tags : [];
      for (const tag of operationTags) {
        tagCounts.set(tag, (tagCounts.get(tag) ?? 0) + 1);
      }
      operations.push({
        path: pathKey,
        method: method.toUpperCase(),
        operationId: operation?.operationId || null,
        tags: operationTags,
        summary: operation?.summary || '',
      });
    }
  }

  operations.sort(
    (left, right) => left.path.localeCompare(right.path) || left.method.localeCompare(right.method),
  );

  return {
    source: {
      crate: 'crates/stateset-http',
      exporter: 'crates/stateset-http/examples/export_openapi.rs',
      specModule: 'crates/stateset-http/src/openapi.rs',
    },
    openapiVersion: spec.openapi,
    title: spec.info?.title || 'StateSet Commerce API',
    version: spec.info?.version || null,
    totalPaths: pathEntries.length,
    totalOperations: operations.length,
    totalSchemas: Object.keys(schemas).length,
    totalTags: tags.length,
    methods: [...methodCounts.entries()]
      .sort((left, right) => left[0].localeCompare(right[0]))
      .map(([name, count]) => ({ name, count })),
    tags: tags
      .map((tag) => ({
        name: tag.name,
        description: tag.description || '',
        count: tagCounts.get(tag.name) ?? 0,
      }))
      .sort((left, right) => left.name.localeCompare(right.name)),
    schemas: Object.keys(schemas).sort(),
    operations,
  };
}

function renderMarkdownInventory(inventory) {
  const summaryRows = [
    ['OpenAPI version', `\`${inventory.openapiVersion}\``],
    ['API title', inventory.title],
    ['API version', `\`${inventory.version}\``],
    ['Paths', String(inventory.totalPaths)],
    ['Operations', String(inventory.totalOperations)],
    ['Schemas', String(inventory.totalSchemas)],
    ['Tags', String(inventory.totalTags)],
  ];
  const methodRows = inventory.methods.map((entry) => [entry.name, String(entry.count)]);
  const tagRows = inventory.tags.map((entry) => [
    `\`${entry.name}\``,
    String(entry.count),
    entry.description || '—',
  ]);
  const operationRows = inventory.operations.map((operation) => [
    `\`${operation.method}\``,
    `\`${operation.path}\``,
    operation.tags.map((tag) => `\`${tag}\``).join(', ') || '—',
    operation.operationId ? `\`${operation.operationId}\`` : '—',
    operation.summary || '—',
  ]);

  return `# Rust OpenAPI Inventory

This page is generated from the live Rust OpenAPI spec exported by \`stateset-http\`.
Do not edit it by hand. Regenerate it with:

\`\`\`bash
node ./scripts/ci/generate_rust_openapi_inventory.mjs
\`\`\`

Machine-readable output lives at \`artifacts/compatibility/rust-openapi-inventory.json\`.

## Summary

${renderMarkdownTable(['Metric', 'Value'], summaryRows)}

## Method Counts

${renderMarkdownTable(['Method', 'Operations'], methodRows)}

## Tag Counts

${renderMarkdownTable(['Tag', 'Operations', 'Description'], tagRows)}

## Operations

${renderMarkdownTable(['Method', 'Path', 'Tags', 'Operation ID', 'Summary'], operationRows)}
`;
}

async function verifyOutput(filePath, expectedContent) {
  const relativePath = path.relative(rootDir, filePath);

  try {
    const actualContent = await readFile(filePath, 'utf8');
    if (!compareStrings(actualContent, expectedContent)) {
      console.error(
        `::error file=${relativePath}::Generated Rust OpenAPI inventory is out of date. Run 'node ./scripts/ci/generate_rust_openapi_inventory.mjs'.`,
      );
      return false;
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : 'unknown error';
    console.error(
      `::error file=${relativePath}::Unable to read generated Rust OpenAPI inventory output (${message}). Run 'node ./scripts/ci/generate_rust_openapi_inventory.mjs'.`,
    );
    return false;
  }

  return true;
}

async function main() {
  const spec = runOpenapiExporter();
  const inventory = buildInventory(spec);
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
      `Rust OpenAPI inventory is up to date (${inventory.totalPaths} paths, ${inventory.totalOperations} operations).`,
    );
    return;
  }

  await mkdir(path.dirname(jsonOutputPath), { recursive: true });
  await mkdir(path.dirname(markdownOutputPath), { recursive: true });
  await writeFile(jsonOutputPath, jsonContent, 'utf8');
  await writeFile(markdownOutputPath, markdownContent, 'utf8');

  console.log(
    `Generated Rust OpenAPI inventory (${inventory.totalPaths} paths, ${inventory.totalOperations} operations).`,
  );
}

await main();
