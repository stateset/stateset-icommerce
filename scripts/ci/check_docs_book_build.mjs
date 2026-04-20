#!/usr/bin/env node

import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '../..');
const bookDir = path.join(rootDir, 'docs/book');

const FORBIDDEN_IMPORTS = ['@stateset/cli/agent-toolkit'];

const EXPECTED_IMPORTS = new Map([
  ['docs/book/getting-started.html', ['@stateset/embedded/openai', '@stateset/embedded/generic']],
  [
    'docs/book/ai-agents.html',
    [
      '@stateset/embedded/openai',
      '@stateset/embedded/generic',
      '@stateset/embedded/langchain',
      '@stateset/embedded/vercel-ai',
      '@stateset/embedded/agent-toolkit',
    ],
  ],
  [
    'docs/book/guides/agent-toolkit.html',
    [
      '@stateset/embedded/openai',
      '@stateset/embedded/generic',
      '@stateset/embedded/langchain',
      '@stateset/embedded/vercel-ai',
      '@stateset/embedded/agent-toolkit',
    ],
  ],
  ['docs/book/guides/mcp-tools.html', ['@stateset/embedded/openai', '@stateset/embedded/generic']],
  ['docs/book/print.html', ['@stateset/embedded/openai', '@stateset/embedded/generic']],
]);

function buildLatestBook() {
  const result = spawnSync('mdbook', ['build', 'docs'], {
    cwd: rootDir,
    encoding: 'utf8',
  });

  if (result.error) {
    console.error(`Failed to run mdbook: ${result.error.message}`);
    process.exit(1);
  }

  if (result.status !== 0) {
    if (result.stdout) {
      process.stdout.write(result.stdout);
    }
    if (result.stderr) {
      process.stderr.write(result.stderr);
    }
    console.error(`mdbook build docs failed with exit code ${result.status}.`);
    process.exit(result.status ?? 1);
  }
}

async function* walkFiles(dirPath) {
  for (const entry of await readdir(dirPath, { withFileTypes: true })) {
    const fullPath = path.join(dirPath, entry.name);
    if (entry.isDirectory()) {
      yield* walkFiles(fullPath);
      continue;
    }
    yield fullPath;
  }
}

async function main() {
  buildLatestBook();

  const filesToScan = [];
  for await (const filePath of walkFiles(bookDir)) {
    if (filePath.endsWith('.html') || filePath.endsWith('.js')) {
      filesToScan.push(filePath);
    }
  }

  const errors = [];
  for (const filePath of filesToScan) {
    const relativePath = path.relative(rootDir, filePath);
    const content = await readFile(filePath, 'utf8');

    for (const forbiddenImport of FORBIDDEN_IMPORTS) {
      if (content.includes(forbiddenImport)) {
        errors.push(`${relativePath}: found forbidden import '${forbiddenImport}' in generated latest-book output.`);
      }
    }
  }

  for (const [relativePath, expectedImports] of EXPECTED_IMPORTS) {
    const filePath = path.join(rootDir, relativePath);
    const content = await readFile(filePath, 'utf8');
    for (const expectedImport of expectedImports) {
      if (!content.includes(expectedImport)) {
        errors.push(`${relativePath}: missing expected import '${expectedImport}' in generated latest-book output.`);
      }
    }
  }

  if (errors.length > 0) {
    for (const error of errors) {
      console.error(`::error::${error}`);
    }
    console.error(`Generated latest-book output failed ${errors.length} import freshness check(s).`);
    process.exit(1);
  }

  console.log(
    `Latest mdBook output is fresh across ${filesToScan.length} generated HTML/JS files and ${EXPECTED_IMPORTS.size} key pages.`,
  );
}

await main();
