#!/usr/bin/env node

import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { getAllStaticMcpToolDefinitions } from '../../cli/src/mcp-server-registry.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '../..');

const DOC_PATHS = [
  'README.md',
  'cli/README.md',
  'bindings/node/README.md',
  'docs/src/getting-started.md',
  'docs/src/ai-agents.md',
  'docs/src/examples.md',
  'docs/src/guides/agent-toolkit.md',
  'docs/src/guides/mcp-tools.md',
  'docs/src/guides/observability.md',
  'docs/src/guides/operations.md',
  'docs/src/appendix/troubleshooting.md',
  'docs/src/payments/base-usdc.md',
  'docs/src/payments/budget.md',
  'docs/src/payments/x402.md',
  'docs/src/a2a/infrastructure.md',
  'docs/src/commerce/customers.md',
  'docs/src/commerce/engagement.md',
  'docs/src/commerce/payments.md',
  'docs/src/commerce/b2b-operations.md',
  'docs/src/advanced/compliance.md',
  'docs/src/guides/autonomous-engine.md',
  'docs/src/concepts/reasoning-loop.md',
  'examples/README.md',
];

const TABLE_TOOL_HEADINGS = new Set(['MCP Tools']);
const INLINE_TOOL_LIST_HEADINGS = new Set(['Read Tools', 'Write Tools', 'Admin Tools', 'Quick Reference']);
const TOOL_NAME_SET = new Set(getAllStaticMcpToolDefinitions().map((tool) => tool.name));
const NUMBERED_TOOL_STEP_REGEX = /^\d+\.\s+`([^`]+)`/;

const LINE_PATTERNS = [
  {
    label: 'tool call',
    regex:
      /\b(?:executeTool|executePaidTool|executeToolWithPayment|getTool|getRawTool)\(\s*['"`]([^'"`]+)['"`]/g,
  },
  {
    label: 'tool property',
    regex: /\btool(?:Name)?\s*:\s*['"`]([^'"`]+)['"`]/g,
  },
  {
    label: 'rollback property',
    regex: /\brollback\s*:\s*['"`]([^'"`]+)['"`]/g,
  },
  {
    label: 'tool comparison',
    regex: /\btool\b\s*===\s*['"`]([^'"`]+)['"`]/g,
  },
];

function normalizeToolName(name) {
  const trimmed = name.trim();
  const prefixedMatch = /^mcp__[^_]+(?:-[^_]+)*__(.+)$/.exec(trimmed);
  return prefixedMatch ? prefixedMatch[1] : trimmed;
}

function isValidToolName(name) {
  return TOOL_NAME_SET.has(normalizeToolName(name));
}

function looksLikeToolName(name) {
  return /^mcp__[^`'"\s]+$/.test(name) || /^[a-z][a-z0-9_]*$/.test(name);
}

function formatHeadingTrail(headings) {
  const trail = headings.filter(Boolean);
  if (trail.length === 0) {
    return 'document';
  }
  return trail.join(' > ');
}

function collectLineMatches(line, pattern) {
  const matches = [];
  for (const match of line.matchAll(pattern.regex)) {
    matches.push(match[1]);
  }
  return matches;
}

async function checkDoc(relativePath) {
  const filePath = path.join(rootDir, relativePath);
  const content = await readFile(filePath, 'utf8');
  const lines = content.split(/\r?\n/);
  const headings = [];
  const errors = [];
  let inFencedBlock = false;

  for (const [index, line] of lines.entries()) {
    const lineNumber = index + 1;
    if (/^```/.test(line)) {
      inFencedBlock = !inFencedBlock;
    }

    const headingMatch = !inFencedBlock ? /^(#{1,6})\s+(.*)$/.exec(line) : null;
    if (headingMatch) {
      const depth = headingMatch[1].length;
      headings.length = depth - 1;
      headings[depth - 1] = headingMatch[2].trim();
    }

    const currentHeading = headings[headings.length - 1] ?? '';
    for (const pattern of LINE_PATTERNS) {
      for (const toolName of collectLineMatches(line, pattern)) {
        if (!isValidToolName(toolName)) {
          errors.push({
            file: relativePath,
            line: lineNumber,
            heading: formatHeadingTrail(headings),
            label: pattern.label,
            toolName,
          });
        }
      }
    }

    if (TABLE_TOOL_HEADINGS.has(currentHeading)) {
      const tableMatch = /^\|\s*`([^`]+)`\s*\|/.exec(line);
      if (tableMatch && !isValidToolName(tableMatch[1])) {
        errors.push({
          file: relativePath,
          line: lineNumber,
          heading: formatHeadingTrail(headings),
          label: 'MCP tool table',
          toolName: tableMatch[1],
        });
      }
    }

    if (INLINE_TOOL_LIST_HEADINGS.has(currentHeading)) {
      for (const tokenMatch of line.matchAll(/`([^`]+)`/g)) {
        const toolName = tokenMatch[1];
        if (!looksLikeToolName(toolName)) {
          continue;
        }
        if (!isValidToolName(toolName)) {
          errors.push({
            file: relativePath,
            line: lineNumber,
            heading: formatHeadingTrail(headings),
            label: 'inline tool list',
            toolName,
          });
        }
      }
    }

    const numberedStepMatch = NUMBERED_TOOL_STEP_REGEX.exec(line);
    if (numberedStepMatch && looksLikeToolName(numberedStepMatch[1]) && !isValidToolName(numberedStepMatch[1])) {
      errors.push({
        file: relativePath,
        line: lineNumber,
        heading: formatHeadingTrail(headings),
        label: 'numbered workflow step',
        toolName: numberedStepMatch[1],
      });
    }
  }

  return errors;
}

async function main() {
  const nestedErrors = await Promise.all(DOC_PATHS.map((relativePath) => checkDoc(relativePath)));
  const errors = nestedErrors.flat();

  if (errors.length > 0) {
    for (const error of errors) {
      console.error(
        `::error file=${error.file},line=${error.line}::Unknown MCP tool reference '${error.toolName}' in ${error.label} under ${error.heading}.`,
      );
    }
    console.error(
      `Checked ${DOC_PATHS.length} docs against the live MCP registry and found ${errors.length} invalid tool reference(s).`,
    );
    process.exit(1);
  }

  console.log(`Doc tool references are valid in ${DOC_PATHS.length} guarded docs.`);
}

await main();
