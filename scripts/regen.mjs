#!/usr/bin/env node
/**
 * Regenerate every committed artifact that CI checks for freshness, in
 * dependency order, in one run.
 *
 *   npm run regen                      # rewrite everything, report what changed
 *   npm run regen -- --skip-rust       # skip generators that compile Rust
 *   npm run regen -- --check           # verify only; exit 1 if anything is stale
 *   npm run regen -- --check --fast    # the pre-push subset (no cargo at all)
 *
 * The list mirrors what `scripts/ci/check_release_hygiene.sh` (Release
 * Hygiene), the Release Governance job, the kernel coverage gate and the
 * freshness tests (cli/test/unit/tool-docs-up-to-date.test.js,
 * bindings/node/test/api-reference.js) verify. When one of those learns a new
 * generated file, add its generator here.
 *
 * `--check` works uniformly: it snapshots every output, runs the generators,
 * compares, and restores the snapshots, so the working tree is left as found.
 */
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

/**
 * @typedef {object} Step
 * @property {string} name
 * @property {string[]} command   argv, run from the repository root
 * @property {string[]} outputs   repository-relative files the step writes
 * @property {'none' | 'metadata' | 'compile'} cargo
 *   `metadata` needs the cargo binary but compiles nothing; `compile` builds Rust.
 */

/** @type {Step[]} Order matters: later steps read earlier steps' outputs. */
const STEPS = [
  {
    // index.d.ts augment block, then tool-descriptors.json and
    // docs/src/api/node-reference.md derived from it. Uses the committed
    // napi-generated index.d.ts; a binding change still needs `npm run build`
    // in bindings/node first, which runs this same postbuild.
    name: 'node binding declarations, tool descriptors, API reference',
    command: ['node', 'bindings/node/scripts/postbuild.mjs'],
    outputs: [
      'bindings/node/index.d.ts',
      'bindings/node/tool-descriptors.json',
      'docs/src/api/node-reference.md',
    ],
    cargo: 'none',
  },
  {
    name: 'CLI tool catalog',
    command: ['node', 'cli/scripts/generate-tool-docs.mjs'],
    outputs: ['cli/docs/TOOLS.md'],
    cargo: 'none',
  },
  {
    name: 'kernel mutation boundary',
    command: ['node', 'scripts/ci/generate_kernel_boundary.mjs'],
    outputs: ['kernel/mutation-boundary.json'],
    cargo: 'none',
  },
  ...[
    ['MCP tool inventory', 'generate_mcp_inventory.mjs', 'mcp-tool-inventory'],
    ['agent inventory', 'generate_agent_inventory.mjs', 'agent-inventory'],
    ['API command coverage', 'generate_api_command_coverage.mjs', 'api-command-coverage'],
    ['binding API inventory', 'generate_binding_api_inventory.mjs', 'binding-api-inventory'],
    ['HTTP gateway inventory', 'generate_http_gateway_inventory.mjs', 'http-gateway-inventory'],
    ['MCP API coverage', 'generate_mcp_api_coverage.mjs', 'mcp-api-coverage'],
  ].map(([name, script, stem]) => inventoryStep(name, script, stem, 'none')),
  inventoryStep(
    'workspace inventory',
    'generate_workspace_inventory.mjs',
    'workspace-inventory',
    'metadata',
  ),
  inventoryStep(
    'Rust OpenAPI inventory',
    'generate_rust_openapi_inventory.mjs',
    'rust-openapi-inventory',
    'compile',
  ),
];

/** Checks with no generator: they validate hand-written sources against the outputs. */
const VERIFY_ONLY = [
  ['kernel coverage', ['node', 'scripts/ci/check_kernel_coverage.mjs']],
  ['doc tool references', ['node', 'scripts/ci/check_doc_tool_refs.mjs']],
  ['workflow job references', ['node', 'scripts/ci/check_workflow_job_refs.mjs']],
];

function inventoryStep(name, script, stem, cargo) {
  return {
    name,
    command: ['node', `scripts/ci/${script}`],
    outputs: [`artifacts/compatibility/${stem}.json`, `docs/src/appendix/${stem}.md`],
    cargo,
  };
}

function parseArgs(argv) {
  const opts = { check: false, skipRust: false, fast: false, quiet: false };
  for (const arg of argv) {
    if (arg === '--check') opts.check = true;
    else if (arg === '--skip-rust') opts.skipRust = true;
    else if (arg === '--fast') opts.fast = true;
    else if (arg === '--quiet') opts.quiet = true;
    else if (arg === '-h' || arg === '--help') {
      process.stdout.write(
        readFileSync(fileURLToPath(import.meta.url), 'utf8').match(/\/\*\*([\s\S]*?)\*\//)[1],
      );
      process.exit(0);
    } else {
      console.error(`regen: unknown argument ${arg}`);
      process.exit(2);
    }
  }
  if (process.env.REGEN_SKIP_RUST === '1') opts.skipRust = true;
  // --fast drops everything that shells out to cargo (compile or metadata).
  if (opts.fast) opts.skipRust = true;
  return opts;
}

function read(rel) {
  const file = path.join(root, rel);
  return existsSync(file) ? readFileSync(file) : null;
}

function digest(buf) {
  return buf === null ? null : createHash('sha256').update(buf).digest('hex');
}

function hasCargo() {
  return spawnSync('cargo', ['--version'], { stdio: 'ignore' }).status === 0;
}

function run(command, quiet) {
  const started = Date.now();
  const result = spawnSync(command[0], command.slice(1), {
    cwd: root,
    stdio: quiet ? ['ignore', 'pipe', 'pipe'] : ['ignore', 'inherit', 'inherit'],
    encoding: 'utf8',
  });
  return {
    ok: result.status === 0,
    ms: Date.now() - started,
    output: quiet ? `${result.stdout ?? ''}${result.stderr ?? ''}` : '',
  };
}

const opts = parseArgs(process.argv.slice(2));
const cargoAvailable = hasCargo();
const log = (line) => {
  if (!opts.quiet) console.log(line);
};

/** @type {Map<string, Buffer | null>} */
const snapshots = new Map();
const restore = () => {
  for (const [rel, buf] of snapshots) {
    if (buf === null) rmSync(path.join(root, rel), { force: true });
    else writeFileSync(path.join(root, rel), buf);
  }
};
if (opts.check) {
  // Leave the tree as found even when interrupted.
  for (const signal of ['SIGINT', 'SIGTERM']) {
    process.on(signal, () => {
      restore();
      process.exit(130);
    });
  }
}

const changed = [];
const failed = [];
const skipped = [];

for (const step of STEPS) {
  const skipReason =
    step.cargo === 'compile' && opts.skipRust
      ? 'compiles Rust (--skip-rust)'
      : step.cargo === 'metadata' && opts.fast
        ? 'needs cargo (--fast)'
        : step.cargo !== 'none' && !cargoAvailable
          ? 'cargo not found'
          : null;
  if (skipReason) {
    skipped.push(`${step.name}: ${skipReason}`);
    log(`- skip  ${step.name} (${skipReason})`);
    continue;
  }
  const before = new Map(step.outputs.map((rel) => [rel, read(rel)]));
  for (const [rel, buf] of before) if (!snapshots.has(rel)) snapshots.set(rel, buf);

  const result = run(step.command, opts.quiet || opts.check);
  if (!result.ok) {
    failed.push({ step, output: result.output });
    log(`x fail  ${step.name} (${(result.ms / 1000).toFixed(1)}s): ${step.command.join(' ')}`);
    if (result.output && !opts.quiet) console.error(result.output.trimEnd());
    continue;
  }
  const stepChanged = step.outputs.filter((rel) => digest(before.get(rel)) !== digest(read(rel)));
  changed.push(...stepChanged);
  const status = stepChanged.length > 0 ? (opts.check ? 'STALE' : 'wrote') : 'ok   ';
  log(
    `${stepChanged.length ? '*' : '-'} ${status} ${step.name} (${(result.ms / 1000).toFixed(1)}s)`,
  );
  for (const rel of stepChanged) log(`         ${rel}`);
}

if (opts.check) restore();

// Verify-only checks run against the (regenerated or restored) tree.
for (const [name, command] of VERIFY_ONLY) {
  const result = run(command, true);
  if (!result.ok) {
    failed.push({ step: { name, command }, output: result.output });
    log(`x fail  ${name}: ${command.join(' ')}`);
    if (!opts.quiet) console.error(result.output.trimEnd());
  } else {
    log(`- ok    ${name} (${(result.ms / 1000).toFixed(1)}s)`);
  }
}

log('');
if (skipped.length > 0 && !opts.quiet) {
  console.log(`Skipped ${skipped.length} step(s); CI still checks them:`);
  for (const line of skipped) console.log(`  - ${line}`);
}
if (opts.check) {
  if (changed.length > 0) {
    console.error(`Stale generated file(s):`);
    for (const rel of changed) console.error(`  - ${rel}`);
    console.error(`Run 'npm run regen' at the repository root and commit the result.`);
  } else if (failed.length === 0) {
    log('All generated artifacts are up to date.');
  }
} else if (changed.length > 0) {
  console.log(`Regenerated ${changed.length} file(s). Stage them with:`);
  console.log(`  git add ${changed.join(' ')}`);
} else {
  console.log('All generated artifacts are up to date.');
}
if (failed.length > 0) {
  console.error(`${failed.length} step(s) failed:`);
  for (const { step, output } of failed) {
    console.error(`  - ${step.name}: ${step.command.join(' ')}`);
    // Quiet mode swallowed the step's own output; show it where it matters.
    if (opts.quiet && output) console.error(output.trimEnd().replace(/^/gm, '      '));
  }
}

process.exit(failed.length > 0 || (opts.check && changed.length > 0) ? 1 : 0);
