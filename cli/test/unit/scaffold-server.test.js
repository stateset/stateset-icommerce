/**
 * Tests for cli/src/scaffold-server.js
 *
 * Covers:
 * - safePath() path traversal prevention
 * - Command injection prevention (allowlist, shell metachar rejection)
 * - createScaffoldMcpServer factory
 * - Tool definitions and listing
 * - Preview mode (allowWrite=false) vs write mode
 * - Source-reading security pattern verification
 * - SCAFFOLD_TOOL_NAMES export
 * - File/directory operations (write_file, read_file, list_files)
 * - Seed database tool
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';

// ---------------------------------------------------------------------------
// Module Import with fallback
// ---------------------------------------------------------------------------

let mod;
let createScaffoldMcpServer;
let SCAFFOLD_TOOL_NAMES;
let moduleLoaded = false;

try {
  mod = await import('../../src/scaffold-server.js');
  createScaffoldMcpServer = mod.createScaffoldMcpServer;
  SCAFFOLD_TOOL_NAMES = mod.SCAFFOLD_TOOL_NAMES;
  moduleLoaded = true;
} catch {
  // @anthropic-ai/claude-agent-sdk may not be available
}

let templatesMod;
let templatesLoaded = false;

try {
  templatesMod = await import('../../src/scaffold-templates.js');
  templatesLoaded = true;
} catch {
  // Module may not load without deps
}

// ---------------------------------------------------------------------------
// Source reading for security pattern verification
// ---------------------------------------------------------------------------

const SRC_PATH = path.resolve(
  new URL('../../src/scaffold-server.js', import.meta.url).pathname,
);
const source = fs.readFileSync(SRC_PATH, 'utf8');

const TEMPLATES_SRC_PATH = path.resolve(
  new URL('../../src/scaffold-templates.js', import.meta.url).pathname,
);
const templateSource = fs.readFileSync(TEMPLATES_SRC_PATH, 'utf8');

// ---------------------------------------------------------------------------
// Temp directory helpers
// ---------------------------------------------------------------------------

let tmpDir;

function makeTmpDir() {
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'scaffold-test-'));
  return tmpDir;
}

function cleanTmpDir() {
  if (tmpDir && fs.existsSync(tmpDir)) {
    fs.rmSync(tmpDir, { recursive: true, force: true });
    tmpDir = null;
  }
}

// ---------------------------------------------------------------------------
// safePath() reimplementation (extracted from source for direct testing)
// ---------------------------------------------------------------------------

function safePath(baseDir, subPath) {
  const realpath = fs.realpathSync.native || fs.realpathSync;

  function resolveRealPathCandidate(targetPath) {
    const absolute = path.resolve(targetPath);
    let probe = absolute;
    const missingSegments = [];

    while (!fs.existsSync(probe)) {
      const parent = path.dirname(probe);
      if (parent === probe) {
        throw new Error(`Path does not have an existing ancestor: ${absolute}`);
      }
      missingSegments.unshift(path.basename(probe));
      probe = parent;
    }

    const realExistingPath = realpath(probe);
    return missingSegments.length === 0
      ? realExistingPath
      : path.join(realExistingPath, ...missingSegments);
  }

  const resolved = path.resolve(baseDir, subPath);
  const base = path.resolve(baseDir);
  const baseReal = resolveRealPathCandidate(base);
  const targetReal = resolveRealPathCandidate(resolved);
  const relative = path.relative(baseReal, targetReal);
  if (
    relative &&
    (relative === '..' || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative))
  ) {
    throw new Error('Path traversal detected: path escapes the working directory');
  }
  return resolved;
}

// ============================================================================
// Source-reading security tests
// ============================================================================

describe('scaffold-server source security patterns', () => {
  it('defines safePath function', () => {
    assert.ok(source.includes('function safePath('), 'safePath must be defined');
  });

  it('safePath checks canonical paths to block symlink escapes', () => {
    assert.ok(
      source.includes('fs.realpathSync'),
      'safePath must resolve real paths to block symlink escapes',
    );
    assert.ok(
      source.includes('path.relative(baseReal, targetReal)'),
      'safePath must compare canonicalized paths with path.relative',
    );
  });

  it('safePath throws on traversal detection', () => {
    assert.ok(
      source.includes('Path traversal detected'),
      'safePath must throw with "Path traversal detected" message',
    );
  });

  it('uses execFileSync (not exec or execSync with shell)', () => {
    assert.ok(
      source.includes('execFileSync'),
      'Must use execFileSync for command execution',
    );
  });

  it('does not use shell: true for command execution', () => {
    // The spawn call does not set shell: true
    const shellTrueCount = (source.match(/shell\s*:\s*true/g) || []).length;
    assert.equal(shellTrueCount, 0, 'Must not use shell: true');
  });

  it('defines ALLOWED_EXECUTABLES allowlist', () => {
    assert.ok(
      source.includes('ALLOWED_EXECUTABLES'),
      'Must define ALLOWED_EXECUTABLES set',
    );
  });

  it('allowlist contains npm and git only', () => {
    for (const cmd of ['npm', 'git']) {
      assert.ok(
        source.includes(`'${cmd}'`),
        `ALLOWED_EXECUTABLES must include '${cmd}'`,
      );
    }
  });

  it('allowlist does NOT contain dangerous or general-purpose runtimes', () => {
    for (const cmd of ['sh', 'bash', 'curl', 'wget', 'rm', 'sudo', 'eval', 'node', 'npx', 'cat', 'ls', 'mkdir']) {
      // Check that they are not in the Set(...) definition
      const setMatch = source.match(/ALLOWED_EXECUTABLES\s*=\s*new Set\(\[([^\]]+)\]\)/);
      if (setMatch) {
        assert.ok(
          !setMatch[1].includes(`'${cmd}'`),
          `ALLOWED_EXECUTABLES must NOT include '${cmd}'`,
        );
      }
    }
  });

  it('defines SHELL_METACHAR_RE regex for metacharacter rejection', () => {
    assert.ok(
      source.includes('SHELL_METACHAR_RE'),
      'Must define SHELL_METACHAR_RE regex',
    );
  });

  it('SHELL_METACHAR_RE covers semicolons, pipes, backticks, and $', () => {
    // Verify the regex contains key dangerous chars
    const reMatch = source.match(/SHELL_METACHAR_RE\s*=\s*\/([^/]+)\//);
    assert.ok(reMatch, 'SHELL_METACHAR_RE must be defined as a regex literal');
    const reBody = reMatch[1];
    for (const ch of [';', '&', '|', '`', '$']) {
      assert.ok(reBody.includes(ch), `SHELL_METACHAR_RE must reject '${ch}'`);
    }
  });

  it('uses safePath for write_file, read_file, list_files, add_page, add_component, add_hook, add_api_route', () => {
    // Count safePath calls in the source
    const safePathCalls = (source.match(/safePath\(/g) || []).length;
    assert.ok(
      safePathCalls >= 7,
      `Expected at least 7 safePath() calls (for file operations), got ${safePathCalls}`,
    );
  });

  it('uses spawn with detached/stdio for background commands', () => {
    assert.ok(source.includes('detached: true'), 'Background spawn must use detached: true');
    assert.ok(source.includes("stdio: 'ignore'"), 'Background spawn must ignore stdio');
    assert.ok(source.includes('child.unref()'), 'Background spawn must unref the child');
  });

  it('sets timeout on execFileSync', () => {
    assert.ok(
      source.includes('timeout: 120000'),
      'execFileSync must have a 120s timeout',
    );
  });

  it('truncates command output to 5000 chars', () => {
    assert.ok(
      source.includes('output.slice(0, 5000)'),
      'Command output must be truncated to 5000 chars',
    );
  });

  it('checks allowWrite before executing write operations', () => {
    // Every mutating tool should check allowWrite
    const allowWriteChecks = (source.match(/if\s*\(\s*!allowWrite\s*\)/g) || []).length;
    assert.ok(
      allowWriteChecks >= 7,
      `Expected at least 7 allowWrite checks, got ${allowWriteChecks}`,
    );
  });

  it('imports spawn from node:child_process (not exec)', () => {
    assert.ok(
      source.includes("import { spawn } from 'node:child_process'"),
      'Must import spawn (not exec) from node:child_process',
    );
  });

  it('does not import exec or execSync at top level', () => {
    // The top-level import should only be spawn, not exec/execSync
    const topImport = source.match(/import\s*\{([^}]+)\}\s*from\s*'node:child_process'/);
    if (topImport) {
      const imported = topImport[1];
      assert.ok(!imported.includes('exec ') && !imported.includes('execSync'),
        'Top-level import should not include exec or execSync');
    }
  });
});

// ============================================================================
// safePath() direct unit tests
// ============================================================================

describe('safePath()', () => {
  beforeEach(() => makeTmpDir());
  afterEach(() => cleanTmpDir());

  it('resolves a simple relative path within base', () => {
    const result = safePath(tmpDir, 'foo/bar.txt');
    assert.equal(result, path.join(tmpDir, 'foo/bar.txt'));
  });

  it('resolves base directory itself', () => {
    const result = safePath(tmpDir, '.');
    assert.equal(result, path.resolve(tmpDir));
  });

  it('throws on .. escape attempt', () => {
    assert.throws(
      () => safePath(tmpDir, '../../../etc/passwd'),
      /Path traversal detected/,
    );
  });

  it('throws on absolute path outside base', () => {
    assert.throws(
      () => safePath(tmpDir, '/etc/passwd'),
      /Path traversal detected/,
    );
  });

  it('throws on encoded traversal with nested ..', () => {
    assert.throws(
      () => safePath(tmpDir, 'foo/../../..'),
      /Path traversal detected/,
    );
  });

  it('allows deeply nested paths within base', () => {
    const result = safePath(tmpDir, 'a/b/c/d/e/f.js');
    assert.ok(result.startsWith(tmpDir));
  });

  it('throws when traversal is disguised via intermediate dirs', () => {
    assert.throws(
      () => safePath(tmpDir, 'valid/../../../escape'),
      /Path traversal detected/,
    );
  });

  it('allows path that contains .. but stays within base', () => {
    const result = safePath(tmpDir, 'foo/../bar.txt');
    assert.equal(result, path.join(tmpDir, 'bar.txt'));
  });

  it('rejects symlink escapes that point outside the working directory', () => {
    fs.symlinkSync('/etc', path.join(tmpDir, 'etc-link'));
    assert.throws(
      () => safePath(tmpDir, 'etc-link/passwd'),
      /Path traversal detected/,
    );
  });

  it('rejects empty subpath that resolves to parent of base', () => {
    // path.resolve('/tmp/scaffold-test-xxx', '..') === '/tmp'
    assert.throws(
      () => safePath(tmpDir, '..'),
      /Path traversal detected/,
    );
  });

  it('handles subPath with leading slash (absolute path)', () => {
    assert.throws(
      () => safePath(tmpDir, '/tmp/other-dir'),
      /Path traversal detected/,
    );
  });
});

// ============================================================================
// SCAFFOLD_TOOL_NAMES export
// ============================================================================

describe('SCAFFOLD_TOOL_NAMES', () => {
  it('source exports SCAFFOLD_TOOL_NAMES array', () => {
    assert.ok(
      source.includes('export const SCAFFOLD_TOOL_NAMES'),
      'Must export SCAFFOLD_TOOL_NAMES',
    );
  });

  it('includes all 13 expected tool names in source', () => {
    const expected = [
      'list_templates',
      'list_page_templates',
      'list_component_templates',
      'create_project',
      'add_page',
      'add_component',
      'add_hook',
      'add_api_route',
      'write_file',
      'read_file',
      'list_files',
      'run_command',
      'seed_database',
    ];
    for (const name of expected) {
      assert.ok(
        source.includes(`'${name}'`),
        `SCAFFOLD_TOOL_NAMES must include '${name}'`,
      );
    }
  });

  it('SCAFFOLD_TOOL_NAMES has exactly 13 entries in source', () => {
    const match = source.match(/SCAFFOLD_TOOL_NAMES\s*=\s*\[([\s\S]*?)\];/);
    assert.ok(match, 'Must find SCAFFOLD_TOOL_NAMES array in source');
    const entries = match[1].match(/'[^']+'/g);
    assert.equal(entries.length, 13, `Expected 13 tool names, got ${entries.length}`);
  });

  // Test the actual export if module loaded
  if (moduleLoaded) {
    it('runtime SCAFFOLD_TOOL_NAMES is an array of 13 strings', () => {
      assert.ok(Array.isArray(SCAFFOLD_TOOL_NAMES));
      assert.equal(SCAFFOLD_TOOL_NAMES.length, 13);
      SCAFFOLD_TOOL_NAMES.forEach((name) => {
        assert.equal(typeof name, 'string');
      });
    });
  }
});

// ============================================================================
// scaffold-templates.js tests
// ============================================================================

describe('scaffold-templates source', () => {
  it('exports TEMPLATES with nextjs, nextjs-minimal, vite-react, astro', () => {
    for (const key of ['nextjs', 'nextjs-minimal', 'vite-react', 'astro']) {
      assert.ok(
        templateSource.includes(`'${key}'`) || templateSource.includes(`${key}:`),
        `TEMPLATES must include '${key}'`,
      );
    }
  });

  it('exports PAGE_TEMPLATES with 6 page types', () => {
    for (const key of ['product-listing', 'product-detail', 'cart', 'checkout', 'account', 'orders']) {
      assert.ok(
        templateSource.includes(`'${key}'`) || templateSource.includes(`${key}:`),
        `PAGE_TEMPLATES must include '${key}'`,
      );
    }
  });

  it('exports COMPONENT_TEMPLATES with 7 component types', () => {
    for (const key of ['product-card', 'product-grid', 'cart-drawer', 'add-to-cart', 'checkout-form', 'header', 'footer']) {
      assert.ok(
        templateSource.includes(`'${key}'`) || templateSource.includes(`${key}:`),
        `COMPONENT_TEMPLATES must include '${key}'`,
      );
    }
  });

  it('exports createPackageJson function', () => {
    assert.ok(templateSource.includes('export function createPackageJson'));
  });

  it('exports generatePageContent function', () => {
    assert.ok(templateSource.includes('export function generatePageContent'));
  });

  it('exports generateComponentContent function', () => {
    assert.ok(templateSource.includes('export function generateComponentContent'));
  });

  it('exports generateHookContent function', () => {
    assert.ok(templateSource.includes('export function generateHookContent'));
  });

  it('exports generateApiRouteContent function', () => {
    assert.ok(templateSource.includes('export function generateApiRouteContent'));
  });

  it('exports generateSeedScript function', () => {
    assert.ok(templateSource.includes('export function generateSeedScript'));
  });
});

// ============================================================================
// Command injection prevention (logic tests)
// ============================================================================

describe('command injection prevention (logic verification)', () => {
  // Replicate the allowlist & metachar logic from the source
  const ALLOWED_EXECUTABLES = new Set(['git', 'npm']);
  const APPROVED_COMMANDS = new Set([
    'git add .',
    'git init',
    'git status',
    'npm ci --ignore-scripts',
    'npm install --ignore-scripts',
    'npm run build',
    'npm run dev',
    'npm run lint',
    'npm run test',
    'npm run typecheck',
  ]);
  const SHELL_METACHAR_RE = /[;&|`$(){}!<>]/;

  function checkCommand(command) {
    const normalized = command.trim().split(/\s+/).filter(Boolean).join(' ');
    const executable = normalized.split(' ')[0];
    if (!ALLOWED_EXECUTABLES.has(executable)) {
      return { allowed: false, reason: 'executable not in allowlist' };
    }
    if (SHELL_METACHAR_RE.test(command)) {
      return { allowed: false, reason: 'shell metacharacters detected' };
    }
    if (!APPROVED_COMMANDS.has(normalized)) {
      return { allowed: false, reason: 'command not in approved list' };
    }
    return { allowed: true };
  }

  it('allows "npm install --ignore-scripts"', () => {
    assert.ok(checkCommand('npm install --ignore-scripts').allowed);
  });

  it('allows "npm run dev"', () => {
    assert.ok(checkCommand('npm run dev').allowed);
  });

  it('allows "git init"', () => {
    assert.ok(checkCommand('git init').allowed);
  });

  it('allows "git add ."', () => {
    assert.ok(checkCommand('git add .').allowed);
  });

  it('rejects "rm -rf /"', () => {
    const result = checkCommand('rm -rf /');
    assert.ok(!result.allowed);
    assert.equal(result.reason, 'executable not in allowlist');
  });

  it('rejects "bash -c whoami"', () => {
    const result = checkCommand('bash -c whoami');
    assert.ok(!result.allowed);
  });

  it('rejects "node scripts/seed.js"', () => {
    const result = checkCommand('node scripts/seed.js');
    assert.ok(!result.allowed);
  });

  it('rejects "npx create-next-app my-store"', () => {
    const result = checkCommand('npx create-next-app my-store');
    assert.ok(!result.allowed);
  });

  it('rejects "curl http://evil.com"', () => {
    const result = checkCommand('curl http://evil.com');
    assert.ok(!result.allowed);
  });

  it('rejects "npm install; rm -rf /" (semicolon chaining)', () => {
    const result = checkCommand('npm install; rm -rf /');
    assert.ok(!result.allowed);
    assert.equal(result.reason, 'shell metacharacters detected');
  });

  it('rejects "npm install && curl evil.com" (double ampersand)', () => {
    const result = checkCommand('npm install && curl evil.com');
    assert.ok(!result.allowed);
    assert.equal(result.reason, 'shell metacharacters detected');
  });

  it('rejects "npm install | cat /etc/passwd" (pipe)', () => {
    const result = checkCommand('npm install | cat /etc/passwd');
    assert.ok(!result.allowed);
    assert.equal(result.reason, 'shell metacharacters detected');
  });

  it('rejects "npm install `whoami`" (backtick)', () => {
    const result = checkCommand('npm install `whoami`');
    assert.ok(!result.allowed);
    assert.equal(result.reason, 'shell metacharacters detected');
  });

  it('rejects "npm install $(whoami)" (dollar paren)', () => {
    const result = checkCommand('npm install $(whoami)');
    assert.ok(!result.allowed);
    assert.equal(result.reason, 'shell metacharacters detected');
  });

  it('rejects "npm install > /dev/null" (redirect)', () => {
    const result = checkCommand('npm install > /dev/null');
    assert.ok(!result.allowed);
    assert.equal(result.reason, 'shell metacharacters detected');
  });

  it('rejects "npm install < /etc/passwd" (input redirect)', () => {
    const result = checkCommand('npm install < /etc/passwd');
    assert.ok(!result.allowed);
    assert.equal(result.reason, 'shell metacharacters detected');
  });

  it('rejects commands outside the approved list even with allowed executables', () => {
    const result = checkCommand('npm install');
    assert.ok(!result.allowed);
    assert.equal(result.reason, 'command not in approved list');
  });
});

// ============================================================================
// Module-level tests (only if SDK loaded)
// ============================================================================

describe('createScaffoldMcpServer', { skip: !moduleLoaded && 'SDK not available' }, () => {
  it('is a function', () => {
    assert.equal(typeof createScaffoldMcpServer, 'function');
  });

  it('creates a server object with default options', () => {
    const server = createScaffoldMcpServer({ workDir: '/tmp' });
    assert.ok(server, 'Server must be created');
    assert.equal(typeof server, 'object');
  });

  it('creates a server object with allowWrite=true', () => {
    const server = createScaffoldMcpServer({ workDir: '/tmp', allowWrite: true });
    assert.ok(server, 'Server must be created');
  });

  it('creates a server with the name "stateset-scaffold"', () => {
    // The source shows name: 'stateset-scaffold'
    assert.ok(source.includes("name: 'stateset-scaffold'"));
  });
});

// ============================================================================
// Server structure source analysis
// ============================================================================

describe('scaffold-server structure', () => {
  it('exports createScaffoldMcpServer as named export', () => {
    assert.ok(source.includes('export function createScaffoldMcpServer'));
  });

  it('exports createScaffoldMcpServer as default export', () => {
    assert.ok(source.includes('export default createScaffoldMcpServer'));
  });

  it('accepts workDir and allowWrite options', () => {
    assert.ok(source.includes('workDir'));
    assert.ok(source.includes('allowWrite'));
  });

  it('defaults workDir to process.cwd()', () => {
    assert.ok(source.includes("workDir = process.cwd()"));
  });

  it('defaults allowWrite to false', () => {
    assert.ok(source.includes('allowWrite = false'));
  });

  it('uses createSdkMcpServer from SDK', () => {
    assert.ok(source.includes('createSdkMcpServer'));
  });

  it('uses zod for schema validation', () => {
    assert.ok(source.includes("from 'zod'"));
    assert.ok(source.includes('z.string()'));
    assert.ok(source.includes('z.enum('));
    assert.ok(source.includes('z.boolean()'));
  });
});

// ============================================================================
// Tool definition source analysis
// ============================================================================

describe('tool definitions in source', () => {
  it('defines list_templates tool', () => {
    assert.ok(source.includes("tool('list_templates'"));
  });

  it('defines list_page_templates tool', () => {
    assert.ok(source.includes("tool('list_page_templates'"));
  });

  it('defines list_component_templates tool', () => {
    assert.ok(source.includes("tool('list_component_templates'"));
  });

  it('defines create_project tool with template enum', () => {
    assert.ok(source.includes("tool(\n        'create_project'") || source.includes("tool('create_project'"));
    // Verify the template enum values
    assert.ok(source.includes("'nextjs', 'nextjs-minimal', 'vite-react', 'astro'"));
  });

  it('defines add_page tool with page type enum', () => {
    assert.ok(source.includes("'add_page'"));
    assert.ok(source.includes("'product-listing'"));
    assert.ok(source.includes("'product-detail'"));
  });

  it('defines add_component tool with component type enum', () => {
    assert.ok(source.includes("'add_component'"));
    assert.ok(source.includes("'product-card'"));
    assert.ok(source.includes("'cart-drawer'"));
  });

  it('defines add_hook tool with hook name enum', () => {
    assert.ok(source.includes("'add_hook'"));
    assert.ok(source.includes("'useCart'"));
    assert.ok(source.includes("'useProducts'"));
  });

  it('defines add_api_route tool with HTTP method enum', () => {
    assert.ok(source.includes("'add_api_route'"));
    assert.ok(source.includes("'GET', 'POST', 'PUT', 'PATCH', 'DELETE'"));
  });

  it('defines write_file tool', () => {
    assert.ok(source.includes("'write_file'"));
  });

  it('defines read_file tool', () => {
    assert.ok(source.includes("'read_file'"));
  });

  it('defines list_files tool', () => {
    assert.ok(source.includes("'list_files'"));
  });

  it('defines run_command tool', () => {
    assert.ok(source.includes("'run_command'"));
  });

  it('defines seed_database tool', () => {
    assert.ok(source.includes("'seed_database'"));
  });
});

// ============================================================================
// Preview mode (allowWrite=false) analysis
// ============================================================================

describe('preview mode behavior in source', () => {
  it('create_project returns preview when allowWrite is false', () => {
    assert.ok(
      source.includes("preview: true") && source.includes("Would create"),
      'create_project must return preview with message',
    );
  });

  it('add_page returns preview when allowWrite is false', () => {
    assert.ok(source.includes("Would create page at"));
  });

  it('add_component returns preview when allowWrite is false', () => {
    assert.ok(source.includes("Would create component at"));
  });

  it('add_hook returns preview when allowWrite is false', () => {
    assert.ok(source.includes("Would create hook at"));
  });

  it('add_api_route returns preview when allowWrite is false', () => {
    assert.ok(source.includes("Would create API route at"));
  });

  it('write_file returns preview when allowWrite is false', () => {
    assert.ok(source.includes("Would write"));
  });

  it('run_command returns preview when allowWrite is false', () => {
    assert.ok(source.includes("Would run:"));
  });

  it('seed_database returns preview when allowWrite is false', () => {
    assert.ok(source.includes("Would seed database at"));
  });

  it('preview results set success: false', () => {
    // Count the pattern "success: false, preview: true"
    const previews = (source.match(/success:\s*false,\s*\n?\s*preview:\s*true/g) || []).length;
    assert.ok(previews >= 7, `Expected at least 7 preview blocks, got ${previews}`);
  });
});

// ============================================================================
// Helper functions in source
// ============================================================================

describe('helper functions in source', () => {
  it('defines ensureDir with recursive mkdir', () => {
    assert.ok(source.includes('function ensureDir('));
    assert.ok(source.includes("{ recursive: true }"));
  });

  it('defines writeFileSync helper', () => {
    assert.ok(source.includes('function writeFileSync('));
    assert.ok(source.includes("fs.writeFileSync"));
  });

  it('defines fileExists helper', () => {
    assert.ok(source.includes('function fileExists('));
    assert.ok(source.includes('fs.existsSync'));
  });

  it('defines readFileSync helper', () => {
    assert.ok(source.includes('function readFileSync('));
    assert.ok(source.includes("fs.readFileSync(filePath, 'utf8')"));
  });

  it('defines result() helper that wraps in MCP format', () => {
    assert.ok(source.includes('function result(data)'));
    assert.ok(source.includes("type: 'text'"));
    assert.ok(source.includes('JSON.stringify(data, null, 2)'));
  });

  it('defines errorResult() helper with isError flag', () => {
    assert.ok(source.includes('function errorResult('));
    assert.ok(source.includes('isError: true'));
  });

  it('defines listFilesInDir with recursive option', () => {
    assert.ok(source.includes('function listFilesInDir('));
    assert.ok(source.includes('withFileTypes: true'));
  });

  it('listFilesInDir skips hidden dirs and node_modules', () => {
    assert.ok(source.includes("entry.name.startsWith('.')"));
    assert.ok(source.includes("entry.name !== 'node_modules'"));
  });
});

// ============================================================================
// create_project structure analysis
// ============================================================================

describe('create_project tool structure', () => {
  it('checks if directory already exists and returns error', () => {
    assert.ok(source.includes('Directory') && source.includes('already exists'));
  });

  it('creates package.json in project directory', () => {
    assert.ok(source.includes("path.join(projectDir, 'package.json')"));
  });

  it('creates tsconfig.json', () => {
    assert.ok(source.includes("path.join(projectDir, 'tsconfig.json')"));
  });

  it('creates next.config.js for Next.js templates', () => {
    assert.ok(source.includes("template.startsWith('next')"));
    assert.ok(source.includes("path.join(projectDir, 'next.config.js')"));
  });

  it('creates Tailwind config when feature is enabled', () => {
    assert.ok(source.includes("features.includes('tailwind')"));
    assert.ok(source.includes("path.join(projectDir, 'tailwind.config.ts')"));
    assert.ok(source.includes("path.join(projectDir, 'postcss.config.js')"));
  });

  it('creates proper directory structure', () => {
    for (const dir of ['app', 'components', 'lib', 'hooks', 'public', 'styles']) {
      assert.ok(
        source.includes(`'${dir}'`),
        `create_project must create '${dir}' directory`,
      );
    }
  });

  it('creates base files (layout, page, styles, gitignore, env, readme)', () => {
    assert.ok(source.includes("'lib/commerce.ts'"));
    assert.ok(source.includes("'app/layout.tsx'"));
    assert.ok(source.includes("'app/page.tsx'"));
    assert.ok(source.includes("'styles/globals.css'"));
    assert.ok(source.includes("'.gitignore'"));
    assert.ok(source.includes("'.env.local'"));
    assert.ok(source.includes("'README.md'"));
  });

  it('returns nextSteps with cd, npm install, npm run dev', () => {
    assert.ok(source.includes('npm install'));
    assert.ok(source.includes('npm run dev'));
  });
});

// ============================================================================
// write_file and read_file behavior
// ============================================================================

describe('write_file tool source behavior', () => {
  it('uses safePath to validate the file path', () => {
    // The write_file handler calls safePath(workDir, filePath)
    const writeSection = source.substring(
      source.indexOf("'write_file'"),
      source.indexOf("'read_file'"),
    );
    assert.ok(writeSection.includes('safePath(workDir, filePath)'));
  });

  it('checks for overwrite flag before writing existing files', () => {
    assert.ok(source.includes('fileExists(fullPath) && !overwrite'));
    assert.ok(source.includes('already exists. Set overwrite: true'));
  });

  it('reports character count in result', () => {
    assert.ok(source.includes('content.length'));
  });
});

describe('read_file tool source behavior', () => {
  it('uses safePath to validate the file path', () => {
    const readSection = source.substring(
      source.indexOf("'read_file'"),
      source.indexOf("'list_files'"),
    );
    assert.ok(readSection.includes('safePath(workDir, filePath)'));
  });

  it('returns error if file does not exist', () => {
    assert.ok(source.includes('does not exist'));
  });
});

// ============================================================================
// Template content tests (from scaffold-templates.js)
// ============================================================================

describe('scaffold-templates content', { skip: !templatesLoaded && 'templates module not available' }, () => {
  it('TEMPLATES has exactly 4 entries', () => {
    assert.equal(Object.keys(templatesMod.TEMPLATES).length, 4);
  });

  it('nextjs template has correct features', () => {
    const t = templatesMod.TEMPLATES.nextjs;
    assert.equal(t.framework, 'next');
    assert.ok(t.features.includes('ssr'));
    assert.ok(t.features.includes('tailwind'));
    assert.ok(t.features.includes('typescript'));
  });

  it('PAGE_TEMPLATES has exactly 6 entries', () => {
    assert.equal(Object.keys(templatesMod.PAGE_TEMPLATES).length, 6);
  });

  it('COMPONENT_TEMPLATES has exactly 7 entries', () => {
    assert.equal(Object.keys(templatesMod.COMPONENT_TEMPLATES).length, 7);
  });

  it('createPackageJson returns valid package object', () => {
    const pkg = templatesMod.createPackageJson('test-store', 'nextjs', []);
    assert.equal(pkg.name, 'test-store');
    assert.ok(pkg.dependencies['@stateset/embedded']);
    assert.ok(pkg.dependencies.next);
    assert.ok(pkg.dependencies.react);
  });

  it('createPackageJson sanitizes name to lowercase kebab', () => {
    const pkg = templatesMod.createPackageJson('My Store!', 'nextjs', []);
    assert.equal(pkg.name, 'my-store-');
  });

  it('createPackageJson adds tailwind devDeps for nextjs template', () => {
    const pkg = templatesMod.createPackageJson('store', 'nextjs', []);
    assert.ok(pkg.devDependencies.tailwindcss);
    assert.ok(pkg.devDependencies.postcss);
    assert.ok(pkg.devDependencies.autoprefixer);
  });

  it('createPackageJson skips tailwind for nextjs-minimal', () => {
    const pkg = templatesMod.createPackageJson('store', 'nextjs-minimal', []);
    assert.ok(!pkg.devDependencies.tailwindcss);
  });

  it('createTsConfig returns valid JSON', () => {
    const config = templatesMod.createTsConfig('nextjs');
    const parsed = JSON.parse(config);
    assert.ok(parsed.compilerOptions);
    assert.equal(parsed.compilerOptions.strict, true);
  });

  it('createNextConfig returns config string', () => {
    const config = templatesMod.createNextConfig();
    assert.ok(config.includes('nextConfig'));
    assert.ok(config.includes('images.unsplash.com'));
  });

  it('generatePageContent returns content for all page types', () => {
    for (const pt of ['product-listing', 'product-detail', 'cart', 'checkout', 'account', 'orders']) {
      const content = templatesMod.generatePageContent(pt);
      assert.ok(content.length > 0, `Page type '${pt}' should generate content`);
    }
  });

  it('generatePageContent returns custom page for unknown type', () => {
    const content = templatesMod.generatePageContent('custom', 'MyPage');
    assert.ok(content.includes('MyPage'));
  });

  it('generateComponentContent returns content for known types', () => {
    for (const ct of ['product-card', 'add-to-cart', 'header', 'footer']) {
      const content = templatesMod.generateComponentContent(ct);
      assert.ok(content.length > 0, `Component type '${ct}' should generate content`);
    }
  });

  it('generateComponentContent returns custom component', () => {
    const content = templatesMod.generateComponentContent('custom', 'Widget');
    assert.ok(content.includes('Widget'));
  });

  it('generateHookContent returns useCart hook', () => {
    const content = templatesMod.generateHookContent('useCart');
    assert.ok(content.includes('useCart'));
    assert.ok(content.includes('addItem'));
  });

  it('generateHookContent returns useProducts hook', () => {
    const content = templatesMod.generateHookContent('useProducts');
    assert.ok(content.includes('useProducts'));
    assert.ok(content.includes('fetchProducts'));
  });

  it('generateHookContent returns custom hook', () => {
    const content = templatesMod.generateHookContent('custom', 'useWidget');
    assert.ok(content.includes('useWidget'));
  });

  it('generateApiRouteContent generates handlers for specified methods', () => {
    const content = templatesMod.generateApiRouteContent('products', ['GET', 'POST']);
    assert.ok(content.includes('export async function GET'));
    assert.ok(content.includes('export async function POST'));
    assert.ok(!content.includes('export async function DELETE'));
  });

  it('generateSeedScript includes product data', () => {
    const script = templatesMod.generateSeedScript('./store.db', 5);
    assert.ok(script.includes("'./store.db'"));
    assert.ok(script.includes('Classic T-Shirt'));
    assert.ok(script.includes('.slice(0, 5)'));
  });

  it('createRootLayout includes project name', () => {
    const layout = templatesMod.createRootLayout('My Store');
    assert.ok(layout.includes('My Store'));
    assert.ok(layout.includes('StateSet iCommerce'));
  });

  it('createHomePage returns valid JSX', () => {
    const page = templatesMod.createHomePage();
    assert.ok(page.includes('HomePage'));
    assert.ok(page.includes('getProducts'));
  });

  it('createGlobalStyles includes tailwind directives', () => {
    const styles = templatesMod.createGlobalStyles();
    assert.ok(styles.includes('@tailwind base'));
    assert.ok(styles.includes('@tailwind components'));
    assert.ok(styles.includes('@tailwind utilities'));
  });

  it('createGitignore includes common patterns', () => {
    const gi = templatesMod.createGitignore();
    assert.ok(gi.includes('node_modules'));
    assert.ok(gi.includes('.next'));
    assert.ok(gi.includes('.env'));
    assert.ok(gi.includes('*.db'));
  });

  it('createReadme includes project name and template info', () => {
    const readme = templatesMod.createReadme('TestStore', 'nextjs');
    assert.ok(readme.includes('TestStore'));
    assert.ok(readme.includes('StateSet iCommerce'));
    assert.ok(readme.includes('npm install'));
  });

  it('createEnvLocal includes DATABASE_PATH', () => {
    const env = templatesMod.createEnvLocal();
    assert.ok(env.includes('DATABASE_PATH'));
  });
});

describe('scaffold-templates source fallback', { skip: templatesLoaded && 'templates loaded, no fallback needed' }, () => {
  it('scaffold-templates source defines TEMPLATES export', () => {
    assert.ok(templateSource.includes('export const TEMPLATES'));
  });
});

// ============================================================================
// seed_database tool analysis
// ============================================================================

describe('seed_database tool source', () => {
  it('defaults dbPath to ./store.db', () => {
    assert.ok(source.includes("dbPath = './store.db'"));
  });

  it('defaults productCount to 10', () => {
    assert.ok(source.includes('productCount = 10'));
  });

  it('writes seed script to scripts/seed.js', () => {
    assert.ok(source.includes("'scripts/seed.js'"));
  });
});

// ============================================================================
// Error handling in source
// ============================================================================

describe('error handling in source', () => {
  it('add_page returns error for invalid page type or missing custom path', () => {
    assert.ok(source.includes('Invalid page type or missing custom path'));
  });

  it('add_component returns error for invalid component type', () => {
    assert.ok(source.includes('Invalid component type or missing custom path'));
  });

  it('add_hook returns error when hook name is required', () => {
    assert.ok(source.includes('Hook name is required'));
  });

  it('run_command catches execution errors and returns errorResult', () => {
    const cmdSection = source.substring(
      source.indexOf("'run_command'"),
      source.indexOf("'seed_database'"),
    );
    assert.ok(cmdSection.includes('catch (error)'));
    assert.ok(cmdSection.includes('errorResult(error.message)'));
  });

  it('write_file checks for existing file without overwrite', () => {
    assert.ok(source.includes('already exists. Set overwrite: true'));
  });

  it('read_file returns error when file does not exist', () => {
    assert.ok(source.includes('does not exist'));
  });

  it('list_files returns error when directory does not exist', () => {
    // Check that it tests for directory existence
    const listSection = source.substring(
      source.indexOf("'list_files'"),
      source.indexOf("'run_command'"),
    );
    assert.ok(listSection.includes('does not exist'));
  });

  it('run_command returns error for disallowed executables', () => {
    assert.ok(source.includes('Command not allowed. Permitted executables:'));
  });

  it('run_command returns error for commands outside the approved list', () => {
    assert.ok(source.includes('Command not allowed. Approved commands:'));
  });

  it('run_command returns error for shell metacharacters', () => {
    assert.ok(source.includes('Command contains disallowed shell metacharacters'));
  });
});
