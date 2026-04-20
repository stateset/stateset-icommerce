#!/usr/bin/env node

import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildMcpApiCoverage } from '../../cli/src/coverage/mcp-api-coverage.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '../..');
const jsonOutputPath = path.join(rootDir, 'artifacts/compatibility/mcp-api-coverage.json');
const markdownOutputPath = path.join(rootDir, 'docs/src/appendix/mcp-api-coverage.md');
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

function renderListSection(title, values, formatter = (value) => `- \`${value}\``) {
  if (values.length === 0) {
    return `## ${title}\n\nNone.\n`;
  }

  return `## ${title}\n\n${values.map((value) => formatter(value)).join('\n')}\n`;
}

function renderMarkdownCoverage(coverage) {
  const summaryRows = [
    ['Domain tool modules', String(coverage.totalDomainModules)],
    ['Domain tools', String(coverage.totalDomainTools)],
    ['Commerce getters', String(coverage.totalCommerceGetters)],
    ['Mapped getters', String(coverage.mappedCommerceGetters)],
    ['Audited classes', String(coverage.totalAuditedClasses)],
    ['Audited methods', String(coverage.totalAuditedMethods)],
    ['Mapped audited methods', String(coverage.mappedAuditedMethods)],
    ['Fully covered', coverage.fullyCovered ? 'yes' : 'no'],
  ];

  const getterRows = coverage.getters.map((entry) => [
    `\`${entry.getter}\``,
    entry.module ? `\`${entry.module}\`` : 'missing',
    String(entry.toolCount),
  ]);

  const auditedRows = coverage.auditedClasses.map((entry) => [
    `\`${entry.className}\``,
    String(entry.methodCount),
    String(entry.mappedMethodCount),
    String(entry.uncoveredMethods.length),
    String(entry.staleMappedMethods.length),
    String(entry.invalidToolReferences.length),
  ]);

  return `# MCP API Coverage

This page is generated from the Node binding surface in \`bindings/node/index.d.ts\`
and the shared MCP coverage model in \`cli/src/coverage/mcp-api-coverage.js\`.
Do not edit it by hand. Regenerate it with:

\`\`\`bash
node ./scripts/ci/generate_mcp_api_coverage.mjs
\`\`\`

Machine-readable output lives at \`artifacts/compatibility/mcp-api-coverage.json\`.

## Summary

${renderMarkdownTable(['Metric', 'Value'], summaryRows)}

## Commerce Getter Coverage

${renderMarkdownTable(['Getter', 'Mapped module', 'Tools'], getterRows)}

## Audited Class Coverage

${renderMarkdownTable(
  ['Class', 'Methods', 'Mapped', 'Uncovered', 'Stale mappings', 'Invalid tool refs'],
  auditedRows,
)}

${renderListSection('Uncovered Commerce Getters', coverage.uncoveredCommerceGetters)}
${renderListSection('Stale Getter Mappings', coverage.staleGetterMappings)}
${renderListSection('Uncovered Audited Methods', coverage.uncoveredAuditedMethods)}
${renderListSection('Stale Audited Method Mappings', coverage.staleAuditedMethodMappings)}
${renderListSection('Invalid Audited Tool References', coverage.invalidAuditedToolReferences)}
`;
}

async function verifyOutput(filePath, expectedContent) {
  const relativePath = path.relative(rootDir, filePath);

  try {
    const actualContent = await readFile(filePath, 'utf8');
    if (!compareStrings(actualContent, expectedContent)) {
      console.error(
        `::error file=${relativePath}::Generated MCP API coverage is out of date. Run 'node ./scripts/ci/generate_mcp_api_coverage.mjs'.`,
      );
      return false;
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : 'unknown error';
    console.error(
      `::error file=${relativePath}::Unable to read generated MCP API coverage output (${message}). Run 'node ./scripts/ci/generate_mcp_api_coverage.mjs'.`,
    );
    return false;
  }

  return true;
}

async function main() {
  const coverage = buildMcpApiCoverage();
  const jsonContent = `${JSON.stringify(coverage, null, 2)}\n`;
  const markdownContent = renderMarkdownCoverage(coverage);

  if (checkMode) {
    const ok = await Promise.all([
      verifyOutput(jsonOutputPath, jsonContent),
      verifyOutput(markdownOutputPath, markdownContent),
    ]);

    if (!ok.every(Boolean)) {
      process.exit(1);
    }

    console.log(
      `MCP API coverage is up to date (${coverage.totalAuditedMethods} audited methods, fullyCovered=${coverage.fullyCovered}).`,
    );
    return;
  }

  await mkdir(path.dirname(jsonOutputPath), { recursive: true });
  await mkdir(path.dirname(markdownOutputPath), { recursive: true });
  await writeFile(jsonOutputPath, jsonContent, 'utf8');
  await writeFile(markdownOutputPath, markdownContent, 'utf8');

  console.log(
    `Generated MCP API coverage (${coverage.totalAuditedMethods} audited methods, fullyCovered=${coverage.fullyCovered}).`,
  );
}

await main();
