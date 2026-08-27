#!/usr/bin/env node

import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { classifyKernelToolBoundary } from '../../cli/src/kernel-boundary.js';
import { AGENTIC_RUNTIME_TOOLS } from '../../cli/src/mcp/agentic-runtime-tools.js';
import { ALL_DOMAIN_TOOLS } from '../../cli/src/tools/domain-registry.js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const outputPath = path.join(root, 'kernel/mutation-boundary.json');
const report = classifyKernelToolBoundary([...ALL_DOMAIN_TOOLS, ...AGENTIC_RUNTIME_TOOLS]);

await writeFile(outputPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
process.stdout.write(
  `kernel boundary: ${report.counts.mutations} mutations classified ` +
    `(${report.counts.governed} governed, ${report.counts.governedComposite} governed composite, ` +
    `${report.counts.blocked} blocked)\n`,
);
