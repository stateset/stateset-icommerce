// Checks claims the docs make against the code they describe, so a renamed
// tool, a removed profile or a broken `npx` line fails here instead of in a
// reader's terminal.
//
//   (a) tools named by cli/.claude/agents/*.md exist (domain registry, or the
//       scaffold server for agents that drive it)
//   (b) every `--profile <value>` is a real MCP tool profile
//   (c) every `npx` command that runs an `@stateset/cli` bin uses `-p`
//   (d) the tool-catalog domain count quoted in cli/README.md is the real one

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

import { ALL_DOMAIN_TOOLS, DOMAIN_TOOL_ENTRIES } from '../../src/tools/domain-registry.js';
import { MCP_TOOL_PROFILES } from '../../src/mcp/tool-profiles.js';
import { SCAFFOLD_TOOL_NAMES } from '../../src/scaffold-server.js';

const CLI_ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const REPO_ROOT = join(CLI_ROOT, '..');
const rel = (path) => relative(REPO_ROOT, path);

function markdownFilesUnder(dir) {
  return readdirSync(dir, { withFileTypes: true, recursive: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith('.md'))
    .map((entry) => join(entry.parentPath, entry.name))
    .sort();
}

/** Files whose `--profile` values and `npx` commands are checked. */
const DOC_FILES = [
  join(REPO_ROOT, 'server.json'),
  join(REPO_ROOT, 'llms.txt'),
  join(CLI_ROOT, 'README.md'),
  ...markdownFilesUnder(join(REPO_ROOT, 'docs', 'src')),
].filter((path) => existsSync(path));

const lineOf = (text, index) => text.slice(0, index).split('\n').length;
const lineText = (text, index) => text.split('\n')[lineOf(text, index) - 1].trim();

function report(problems, heading) {
  assert.equal(problems.length, 0, `${heading}:\n  ${problems.join('\n  ')}`);
}

// ---------------------------------------------------------------------------
// (a) agent tool lists
// ---------------------------------------------------------------------------

const REGISTERED_TOOLS = new Set(ALL_DOMAIN_TOOLS.map((tool) => tool.name));

/**
 * Where a tool name may resolve, by MCP server. Agent frontmatter names tools
 * as `mcp__<server>__<tool>`; a bare name (storefront.md's "Available Tools"
 * list) may come from any listed source.
 */
const TOOL_SOURCES = {
  'stateset-commerce': REGISTERED_TOOLS, // cli/src/tools/domain-registry.js
  'stateset-scaffold': new Set(SCAFFOLD_TOOL_NAMES), // cli/src/scaffold-server.js
};

/** @returns {{ name: string, line: number }[]} tools an agent file lists */
function listedTools(text) {
  const lines = text.split('\n');
  const found = [];
  if (lines[0] === '---') {
    const end = lines.indexOf('---', 1);
    for (let i = 1; i < end; i += 1) {
      if (!lines[i].startsWith('tools:')) continue;
      for (const name of lines[i].slice('tools:'.length).split(',')) {
        if (name.trim()) found.push({ name: name.trim(), line: i + 1 });
      }
    }
    return found;
  }
  // No frontmatter: bullets (`- \`tool_name\` - what it does`) under "## Available Tools".
  let inSection = false;
  lines.forEach((line, i) => {
    if (line.startsWith('## ')) inSection = line.trim() === '## Available Tools';
    const bullet = inSection && line.match(/^- `([a-z0-9_]+)`/);
    if (bullet) found.push({ name: bullet[1], line: i + 1 });
  });
  return found;
}

function resolvesToRealTool(name) {
  const qualified = name.match(/^mcp__(.+?)__(.+)$/);
  if (qualified) return TOOL_SOURCES[qualified[1]]?.has(qualified[2]) ?? false;
  if (!/^[a-z0-9_]+$/.test(name)) return true; // built-in harness tool (Read, Bash, ...)
  return Object.values(TOOL_SOURCES).some((source) => source.has(name));
}

test('agent files only name tools that exist', () => {
  const agentsDir = join(CLI_ROOT, '.claude', 'agents');
  const problems = [];
  for (const file of markdownFilesUnder(agentsDir)) {
    const tools = listedTools(readFileSync(file, 'utf8'));
    if (tools.length === 0) problems.push(`${rel(file)}: no tool list found`);
    for (const { name, line } of tools) {
      if (!resolvesToRealTool(name)) {
        problems.push(`${rel(file)}:${line}: unknown tool "${name}"`);
      }
    }
  }
  report(
    problems,
    'Agent files name tools that are in neither the domain registry nor the scaffold server',
  );
});

// ---------------------------------------------------------------------------
// (b) --profile values
// ---------------------------------------------------------------------------

const PROFILE_NAMES = Object.keys(MCP_TOOL_PROFILES);

test('every --profile value in the docs is a real profile', () => {
  const problems = [];
  // `--profile core`, `--profile=core`, and the JSON-args form `"--profile", "core"`
  // (but not a JSON value followed by the next key: `"name": "--profile", "description"`).
  const pattern = /(?<!:\s*")--profile(?:=|[ \t]+|"\s*,\s*")([A-Za-z0-9_-]+)/g;
  for (const file of DOC_FILES) {
    const text = readFileSync(file, 'utf8');
    for (const match of text.matchAll(pattern)) {
      if (!PROFILE_NAMES.includes(match[1])) {
        problems.push(
          `${rel(file)}:${lineOf(text, match.index)}: "--profile ${match[1]}" in: ${lineText(text, match.index)}`,
        );
      }
    }
  }
  report(problems, `Unknown --profile value (real profiles: ${PROFILE_NAMES.join(', ')})`);
});

test('server.json describes --profile with real profile names', () => {
  const file = join(REPO_ROOT, 'server.json');
  const text = readFileSync(file, 'utf8');
  const args = JSON.parse(text).packages.flatMap((pkg) => pkg.packageArguments ?? []);
  const profileArg = args.find((arg) => arg.name === '--profile');
  assert.ok(profileArg, 'server.json: no --profile package argument');

  const line = lineOf(text, text.indexOf(profileArg.description));
  const where = `${rel(file)}:${line}`;
  assert.ok(
    PROFILE_NAMES.includes(profileArg.default),
    `${where}: default "${profileArg.default}" is not a profile (${PROFILE_NAMES.join(', ')})`,
  );
  // The description is "<label>: name (gloss), name (gloss), ... or name (gloss)".
  const named = profileArg.description
    .slice(profileArg.description.indexOf(':') + 1)
    .replace(/\([^)]*\)/g, '')
    .split(/,|\bor\b/)
    .map((part) => part.trim())
    .filter(Boolean);
  assert.ok(named.length > 0, `${where}: description names no profiles: ${profileArg.description}`);
  const unknown = named.filter((name) => !PROFILE_NAMES.includes(name));
  assert.deepEqual(
    unknown,
    [],
    `${where}: description names "${unknown.join('", "')}", not a profile (${PROFILE_NAMES.join(', ')}): ${profileArg.description}`,
  );
});

// ---------------------------------------------------------------------------
// (c) npx commands
// ---------------------------------------------------------------------------

const cliPackage = JSON.parse(readFileSync(join(CLI_ROOT, 'package.json'), 'utf8'));
const BIN_NAMES = Object.keys(cliPackage.bin);
// `npx <pkg> <args>` runs the bin named after the package (unscoped). A package
// with several bins and none of that name has no default: `npx @stateset/cli x`
// fails, and `-p @stateset/cli x` is the only working form.
const DEFAULT_BIN = BIN_NAMES.find((bin) => bin === cliPackage.name.split('/').pop());

test('npx commands that run an @stateset/cli bin use -p', () => {
  const problems = [];
  for (const file of DOC_FILES) {
    const text = readFileSync(file, 'utf8');
    const flag = (found, index) =>
      problems.push(`${rel(file)}:${lineOf(text, index)}: ${found} in: ${lineText(text, index)}`);

    // Shell form: npx [flags] @stateset/cli[@version] <bin> ...
    for (const match of text.matchAll(/\bnpx\b[^\n`|;&]*/g)) {
      const words = match[0].split(/\s+/);
      const at = words.findIndex((word) => /^@stateset\/cli(@[^/]*)?$/.test(word));
      if (at === -1) continue;
      const bin = words[at + 1];
      const usesPackageFlag = ['-p', '--package'].includes(words[at - 1]);
      if (usesPackageFlag) {
        if (bin && !bin.startsWith('-') && !BIN_NAMES.includes(bin)) {
          flag(`"${bin}" is not a bin of ${cliPackage.name}`, match.index);
        }
      } else if (bin !== DEFAULT_BIN || DEFAULT_BIN === undefined) {
        flag(`npx without -p (${cliPackage.name} has no default bin)`, match.index);
      }
    }

    // JSON-args form: "args": [..., "@stateset/cli", "stateset-mcp", ...]
    // (an array element, so not `"identifier": "@stateset/cli", "version"`).
    const jsonArgs = /("[^"]*"\s*,\s*)?(?<!:\s*)"@stateset\/cli(@[^"/]*)?"\s*,\s*"([^"]+)"/g;
    for (const match of text.matchAll(jsonArgs)) {
      const usesPackageFlag = /^"(-p|--package)"/.test(match[1] ?? '');
      const bin = match[3];
      if (!BIN_NAMES.includes(bin)) {
        flag(`"${bin}" is not a bin of ${cliPackage.name}`, match.index);
      } else if (!usesPackageFlag && bin !== DEFAULT_BIN) {
        flag('npx args without "-p"', match.index);
      }
    }
  }
  report(problems, 'npx invocations of @stateset/cli that cannot run as written');
});

// ---------------------------------------------------------------------------
// (d) domain count
// ---------------------------------------------------------------------------

test('cli/README.md quotes the real tool-catalog domain count', () => {
  const file = join(CLI_ROOT, 'README.md');
  const text = readFileSync(file, 'utf8');
  const matches = [...text.matchAll(/(\d+) domains total/g)];
  assert.ok(matches.length > 0, `${rel(file)}: no "<n> domains total" claim found`);
  for (const match of matches) {
    assert.equal(
      Number(match[1]),
      DOMAIN_TOOL_ENTRIES.length,
      `${rel(file)}:${lineOf(text, match.index)}: says ${match[1]} domains, the registry has ${DOMAIN_TOOL_ENTRIES.length}: ${lineText(text, match.index)}`,
    );
  }
});
