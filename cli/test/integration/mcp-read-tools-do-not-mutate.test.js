/**
 * A tool that says `permission: 'read'` must not change persisted state.
 *
 * Kernel strict mode -- the governed write path for autonomous agents --
 * decides what to expose from each tool's self-declared `permission`: writes
 * must go through a typed kernel command, reads are exposed as they are
 * (src/kernel-boundary.js). A read tool that writes is therefore an
 * ungoverned write path. Nothing else checks the declaration against what the
 * handler does.
 *
 * This test seeds a store (the demo data `stateset-mcp-http` boots with, then
 * a list of write tools: test/helpers/read-tool-sweep.js SEED_WRITES, with
 * NATIVE_SEEDS for records only governed tools create), then calls every read
 * tool through the server's own dispatch (`server.executeTool`: validation,
 * policy, permission, then the handler -- the path scripts/mcp-tool-smoke.mjs
 * drives) in apply mode, the mode a strict endpoint runs in. Inputs start from
 * the smoke sampler's minimal input and add, where they differ: real ids from
 * the seeded store, flipped booleans (`dryRun: false`), every optional field,
 * unique free text, and hand-written fixtures the sampler cannot produce,
 * most of them built from seeded rows (read-tool-sweep.js FIXTURE_INPUTS).
 * After the sweep, LATE_SEED_WRITES (seeds that would otherwise absorb a read
 * tool's first-use init) run and every read tool that has not yet run to
 * success is swept again.
 *
 * A read call that fails proves nothing about what its handler writes, so the
 * test also tracks reach: every read tool must run to success at least once,
 * except those listed under `unreached` in the baseline file, each with the
 * external service, wallet, governed seed or defect that keeps it from
 * running. The sweep stays local: outbound connections and child processes
 * other than Node are refused and reported (read-tool-sweep.js
 * installSweepGuard), and external credentials are removed from the
 * environment.
 *
 * After every call it fingerprints all persisted state under one sandbox that
 * HOME, the working directory, the temp dir and the store all point into
 * (test/helpers/state-fingerprint.js):
 *
 *   - every SQLite database, by content (schema + every row): the commerce
 *     store, which also holds the A2A tables, its audit store, and any
 *     database a tool creates;
 *   - every other file and directory: ~/.stateset, ./.stateset (the sync key
 *     store is a relative path), files written to a path a tool was given (the
 *     sampler points path-like arguments into the sandbox temp dir), and the
 *     policy store and A2A checkpoints beside the database.
 *
 * Excluded, and nothing else: the MCP replay log's record of the call being
 * measured (`<db dir>/.stateset/agentic-tool-calls.jsonl` lines whose `tool`
 * is that call's tool), which every call appends by design. Any other append
 * to that log, or a rewrite of it, is a change. SQLite -wal/-shm files are read
 * through the database rather than hashed. In-memory state (event
 * subscriptions, the replay ring buffer, mock shipping labels) is not
 * persisted and is not measured. The fingerprint is taken from a child process
 * (test/helpers/state-probe.js explains why measuring in-process would break
 * the server's SQLite locks).
 *
 * A refusal is fine -- inputs are synthetic. Only a state change is a
 * violation. Known violations live in
 * test/fixtures/read-tool-mutation-baseline.json, each with a `kind`:
 * `write` (changes rows of the `tables` it lists) or `init` (creates only empty
 * structure on first use: a table or index, a database with no rows, a
 * directory). The list only shrinks: a new violation fails, an `init` tool
 * that changes more than structure fails, a `write` tool that changes anything
 * but rows of its listed tables fails, and a listed tool that no longer
 * violates, a listed table it no longer writes, or a listed tool that is no
 * longer a read tool, fails.
 *
 * A fake read tool that writes a customer row is registered for this process
 * only and swept first and last; the test fails unless both runs are caught,
 * so a fingerprint that stops seeing writes part-way cannot pass silently.
 *
 *   READ_TOOL_SWEEP_ONLY=tool_a,tool_b   sweep only these tools (the negative
 *                                        control always runs). Completeness and
 *                                        stale-baseline checks narrow to them.
 *                                        First-use `init` work happens in
 *                                        whichever tool of a domain runs first,
 *                                        so a narrowed run can attribute it to
 *                                        a different tool than the full sweep.
 *   READ_TOOL_SWEEP_REPORT=out.json      write every call and violation as JSON
 *   READ_TOOL_SWEEP_KEEP=1               keep the sandbox directory
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import childProcess from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import net from 'node:net';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  FIXTURE_INPUTS,
  LATE_SEED_WRITES,
  createSamplerView,
  createSandbox,
  createSeedWriter,
  createVariantBuilder,
  installSweepGuard,
  seedStore,
  sweepTools,
} from '../helpers/read-tool-sweep.js';
import { describeChange } from '../helpers/state-fingerprint.js';
import { createStateProbe } from '../helpers/state-probe.js';

const NEGATIVE_CONTROL = '__read_tool_mutation_negative_control';

const baselineFile = new URL('../fixtures/read-tool-mutation-baseline.json', import.meta.url);
const baseline = JSON.parse(readFileSync(baselineFile, 'utf8'));
const known = new Map(baseline.violations.map((entry) => [entry.tool, entry]));
const unreachable = new Map(baseline.unreached.map((entry) => [entry.tool, entry]));

/**
 * Which read tools reach a successful handler run depends on the engine
 * binding: records a seed creates, inputs a handler accepts. The per-tool
 * reach check runs only against this checkout's own binding (CI builds it in
 * bindings/node); with @stateset/embedded resolved anywhere else -- a
 * node_modules borrowed from another checkout -- this says why it is skipped.
 */
function foreignEngine() {
  try {
    const resolved = realpathSync(createRequire(import.meta.url).resolve('@stateset/embedded'));
    const own = realpathSync(fileURLToPath(new URL('../../../bindings/node/', import.meta.url)));
    return resolved.startsWith(`${own}${path.sep}`)
      ? null
      : `@stateset/embedded resolves to ${resolved}, not this checkout's bindings/node`;
  } catch (error) {
    return `cannot resolve @stateset/embedded: ${error.message}`;
  }
}

const only = process.env.READ_TOOL_SWEEP_ONLY
  ? new Set(
      process.env.READ_TOOL_SWEEP_ONLY.split(',')
        .map((name) => name.trim())
        .filter(Boolean),
    )
  : null;

/** Empty structure created on first use: no row, file content or log line. */
const isInitializationOnly = (changes) =>
  changes.every(
    (change) =>
      (change.kind === 'schema' && change.change === 'added') ||
      (change.kind === 'sqlite' && change.change === 'added' && change.rows === 0) ||
      (change.kind === 'dir' && change.change === 'added'),
  );

/** Changes a `write` baseline entry does not cover: anything but rows of its listed tables. */
const outsideListedTables = (entry, changes) =>
  changes.filter((change) => !(change.kind === 'rows' && entry.tables.includes(change.table)));

/**
 * Claims to be a read; writes a customer. Registered for this process only
 * (pushed onto the agentic runtime tool list before the server module loads,
 * so the real dispatch serves it).
 */
const negativeControl = {
  name: NEGATIVE_CONTROL,
  description: 'Test-only negative control: declared read, writes a customer row.',
  inputSchema: {},
  permission: 'read',
  policyDomain: 'agentic',
  handler: async ({ commerce }) => {
    const customer = await commerce.customers.create({
      email: `negative-control-${randomUUID()}@example.com`,
      firstName: 'Negative',
      lastName: 'Control',
    });
    return { success: true, id: customer.id };
  },
};

async function runSweep(sandbox) {
  const startedAt = Date.now();
  // Before anything imports src/mcp-server.js (the smoke sampler does).
  const { AGENTIC_RUNTIME_TOOLS } = await import('../../src/mcp/agentic-runtime-tools.js');
  const agenticReadTools = AGENTIC_RUNTIME_TOOLS.filter((tool) => tool.permission === 'read');
  AGENTIC_RUNTIME_TOOLS.push(negativeControl);

  const [
    { sample, sampleShape },
    { z },
    { createStatesetMcpServer },
    { getCommerce },
    { seedDemoData },
    { ALL_DOMAIN_TOOLS },
    { DEFAULT_REPLAY_LOG_FILE },
  ] = await Promise.all([
    import('../../scripts/mcp-tool-smoke.mjs'),
    import('zod'),
    import('../../src/mcp-server.js'),
    import('../../src/database.js'),
    import('../../src/seeds/demo.js'),
    import('../../src/tools/domain-registry.js'),
    import('../../src/mcp/replay-log.js'),
  ]);

  // Every read of the sandbox happens in this child process.
  const replayLog = path.join(sandbox.db, '.stateset', DEFAULT_REPLAY_LOG_FILE);
  const probe = await createStateProbe({ roots: [sandbox.root], appendOnly: [replayLog] });
  const server = createStatesetMcpServer({
    dbPath: sandbox.dbPath,
    allowApply: true,
    toolProfile: 'all',
  });
  try {
    await server.servicesReady;
    const invoke = (name, params) => server.executeTool(name, params);

    const { native, view } = createSamplerView(sample, z);
    let pool = null;
    // Values seeds capture from their results (ids the store does not hold).
    const captured = {};
    const refreshPool = async () => {
      const read = await probe.seedPool(sandbox.dbPath);
      pool = { ...read, named: { ...read.named, ...captured } };
    };
    await refreshPool();
    const variantsFor = createVariantBuilder({
      sample,
      sampleShape,
      view,
      pool: () => pool,
      fixtures: FIXTURE_INPUTS,
    });

    const toolsByName = new Map(
      [...ALL_DOMAIN_TOOLS, ...agenticReadTools].map((tool) => [tool.name, tool]),
    );
    const seeding = await seedStore({
      commerce: getCommerce(sandbox.dbPath),
      seedDemoData,
      invoke,
      variantsFor,
      toolsByName,
      refreshPool,
      pool: () => pool,
      remember: (values) => Object.assign(captured, values),
    });
    const seedingRefused = guard.take();

    // Nothing may change state while nothing is called, or every later
    // attribution is suspect.
    await probe.mark();
    await new Promise((resolve) => setTimeout(resolve, 300));
    const quiet = await probe.diff(null);

    // Agentic runtime read tools first, then the domain read tools.
    // First-use `init` work is attributed to whichever tool of a domain runs
    // first, so reordering moves `init` baseline entries.
    const expectedReadTools = [
      ...agenticReadTools,
      ...ALL_DOMAIN_TOOLS.filter((tool) => tool.permission === 'read'),
    ];
    const selected = expectedReadTools.filter((tool) => !only || only.has(tool.name));
    const sweepOptions = {
      invoke,
      changesSince: (toolName) => probe.diff(toolName),
      takeRefused: guard.take,
    };
    const first = await sweepTools({
      ...sweepOptions,
      tools: [negativeControl, ...selected],
      variantsFor,
    });

    // LATE_SEED_WRITES would have absorbed a read tool's first-use init had
    // they run before the sweep. Run them now, then sweep again every read
    // tool no input got through to a successful handler run, in case it was
    // waiting for one of those rows. The negative control closes the sweep.
    const late = await createSeedWriter({
      invoke,
      variantsFor,
      toolsByName,
      refreshPool,
      pool: () => pool,
      remember: (values) => Object.assign(captured, values),
    }).writeAll(LATE_SEED_WRITES);
    const lateRefused = guard.take();
    await probe.mark();
    const ran = new Set(first.calls.filter((call) => call.outcome === 'ok').map((c) => c.tool));
    const second = await sweepTools({
      ...sweepOptions,
      tools: [...selected.filter((tool) => !ran.has(tool.name)), negativeControl],
      variantsFor: (tool) =>
        variantsFor(tool).map((variant) => ({ ...variant, label: `late ${variant.label}` })),
    });
    return {
      native,
      seeding: { ...seeding, late, refused: [...seedingRefused, ...lateRefused] },
      seededTables: [...pool.idsByTable.keys()].sort(),
      quiet,
      expectedReadTools: expectedReadTools.map((tool) => tool.name),
      calls: [...first.calls, ...second.calls],
      violations: [...first.violations, ...second.violations],
      ms: Date.now() - startedAt,
    };
  } finally {
    server.dispose?.();
    await server.close?.().catch(() => {});
    await probe.close();
  }
}

/**
 * The guard must still be refusing at the end of the sweep: a spawn of cargo
 * and a TCP connection, both of which a read tool could otherwise reach.
 */
async function checkGuard() {
  const refusals = [];
  try {
    childProcess.execFileSync('cargo', ['--version'], { stdio: 'ignore' });
  } catch (error) {
    refusals.push(error.code === 'EACCES' && /read-tool sweep/.test(error.message));
  }
  refusals.push(
    await new Promise((resolve) => {
      const socket = net.connect(9, '127.0.0.1');
      socket.once('connect', () => {
        socket.destroy();
        resolve(false);
      });
      socket.once('error', (error) => resolve(/read-tool sweep/.test(error.message)));
    }),
  );
  return { refusals, recorded: guard.take().map((attempt) => attempt.kind) };
}

let sweep = null;
let guardCheck = null;
let unavailable = null;
const sandbox = createSandbox();
// Before anything imports src/: some modules bind child_process helpers at load.
const guard = installSweepGuard();
try {
  sweep = await runSweep(sandbox);
  guardCheck = await checkGuard();
} catch (error) {
  unavailable = error;
  console.warn(`[read-tool-sweep] could not run: ${error.stack ?? error.message}`);
} finally {
  guard.restore();
  sandbox.restore({ keep: Boolean(process.env.READ_TOOL_SWEEP_KEEP) });
}

const describeViolation = (v) =>
  `${v.tool} [${v.variant}] ${JSON.stringify(v.params)} -> ` +
  v.changes.map((change) => describeChange(change, sandbox.root)).join('; ');

if (sweep) {
  const realViolations = sweep.violations.filter((v) => v.tool !== NEGATIVE_CONTROL);
  const violating = new Set(realViolations.map((v) => v.tool));
  const tools = new Set(sweep.calls.map((call) => call.tool).filter((t) => t !== NEGATIVE_CONTROL));
  const variantCounts = sweep.calls.reduce(
    (acc, call) => ({ ...acc, [call.variant]: (acc[call.variant] ?? 0) + 1 }),
    {},
  );
  console.log(
    `[read-tool-sweep] ${tools.size} read tools, ${sweep.calls.length} calls ` +
      `${JSON.stringify(variantCounts)}, ${realViolations.length} violating calls in ` +
      `${violating.size} tools, ${Math.round(sweep.ms / 1000)}s` +
      (sweep.native ? '' : ' (zod 4: schemas shown to the sampler as zod 3)'),
  );
  for (const v of realViolations) {
    const kind = known.get(v.tool)?.kind ?? 'NEW';
    console.log(`[read-tool-sweep]   ${kind.padEnd(5)} ${describeViolation(v)}`);
  }
  // A tool "ran" when at least one input got through to a handler run that
  // succeeded; only those runs can show what the handler writes.
  const ran = new Set(sweep.calls.filter((call) => call.outcome === 'ok').map((c) => c.tool));
  const notRun = [...tools].filter((tool) => !ran.has(tool));
  console.log(
    `[read-tool-sweep] ${tools.size - notRun.length}/${tools.size} read tools ran to success; ` +
      `not run: ${notRun.map((tool) => (unreachable.has(tool) ? tool : `${tool} (unlisted)`)).join(', ')}`,
  );
  for (const call of sweep.calls.filter((c) => c.refused)) {
    const targets = call.refused.map((attempt) => `${attempt.kind} ${attempt.target}`);
    console.log(
      `[read-tool-sweep]   refused ${call.tool} [${call.variant}]: ${targets.join(', ')}`,
    );
  }
  /** Why a tool never ran: its last handler error, else its last validation refusal. */
  const whyNotRun = (tool) => {
    const calls = sweep.calls.filter((call) => call.tool === tool);
    const handled = calls.filter((call) => call.status && call.status !== 'invalid');
    return (handled.at(-1) ?? calls.at(-1))?.message ?? 'no call';
  };

  if (process.env.READ_TOOL_SWEEP_REPORT) {
    writeFileSync(
      process.env.READ_TOOL_SWEEP_REPORT,
      JSON.stringify(
        {
          ...sweep,
          violations: sweep.violations.map((v) => ({
            ...v,
            changes: v.changes.map((change) => ({
              ...change,
              path: path.relative(sandbox.root, change.path),
              summary: describeChange(change, sandbox.root),
            })),
          })),
        },
        null,
        2,
      ),
    );
  }

  describe('read-only MCP tools do not change persisted state', () => {
    it('reaches the handlers: nearly every tool gets an input past validation', () => {
      const passed = new Set(
        sweep.calls.filter((call) => call.status && call.status !== 'invalid').map((c) => c.tool),
      );
      assert.ok(
        passed.size >= 0.9 * tools.size,
        `only ${passed.size}/${tools.size} read tools got an input past validation; ` +
          'the sampler has gone vacuous and the sweep proves little',
      );
    });

    const engineSkip = foreignEngine();
    it(
      'runs every read tool handler to success, except the tools listed as unreached',
      { skip: engineSkip ?? false },
      () => {
        assert.deepEqual(
          notRun.filter((tool) => !unreachable.has(tool)).map((t) => `${t}: ${whyNotRun(t)}`),
          [],
          'no input got these read tools through to a successful handler run, so the sweep ' +
            'cannot see what they write: seed the rows they read or give them a fixture ' +
            '(test/helpers/read-tool-sweep.js); list them as unreached only if they need ' +
            'something the sandbox cannot have',
        );
        const nowRun = [...unreachable.keys()].filter((tool) => ran.has(tool));
        assert.deepEqual(nowRun, [], 'these now run to success; remove them from `unreached`');
      },
    );

    it('lists every unreached tool once, as a read tool, with a reason', () => {
      for (const entry of baseline.unreached) {
        assert.ok(entry.reason?.trim(), `${entry.tool}: give a reason`);
        assert.ok(
          sweep.expectedReadTools.includes(entry.tool),
          `${entry.tool} is not a read tool; remove it from \`unreached\``,
        );
      }
      assert.equal(unreachable.size, baseline.unreached.length, 'a tool is listed twice');
    });

    it('keeps the sweep local: refuses outbound connections and non-Node processes', () => {
      assert.deepEqual(
        guardCheck?.refusals,
        [true, true],
        'the guard no longer refuses a cargo spawn and a TCP connection at the end of the sweep',
      );
      assert.deepEqual(guardCheck.recorded, ['spawn', 'connect']);
    });

    it('seeds rows for read tools to find', () => {
      for (const table of ['customers', 'products', 'orders', 'payment_links']) {
        assert.ok(sweep.seededTables.includes(table), `no ${table} rows were seeded`);
      }
      assert.equal(
        sweep.seeding.last.ok,
        true,
        `last seed write failed: ${sweep.seeding.last.error}`,
      );
    });

    it('sees no state change while nothing is called', () => {
      assert.deepEqual(
        sweep.quiet.map((change) => describeChange(change, sandbox.root)),
        [],
        'state changed with no tool call in flight; attributions would be unreliable',
      );
    });

    it('catches the negative control (a fake read tool that writes a row), first and last', () => {
      const runs = sweep.calls.filter((call) => call.tool === NEGATIVE_CONTROL);
      assert.deepEqual(
        runs.map((call) => call.outcome),
        ['ok', 'ok'],
        'the negative control runs, cleanly, before and after the sweep',
      );
      const caught = sweep.violations.filter(
        (v) =>
          v.tool === NEGATIVE_CONTROL &&
          v.changes.some((change) => change.kind === 'rows' && change.table === 'customers'),
      );
      assert.equal(
        caught.length,
        2,
        'the fingerprint missed a customers row written by the negative control ' +
          `(caught ${caught.length} of 2): the sweep cannot see writes`,
      );
    });

    it('calls every read tool', { skip: only ? 'narrowed by READ_TOOL_SWEEP_ONLY' : false }, () => {
      const missing = sweep.expectedReadTools.filter((name) => !tools.has(name));
      assert.deepEqual(missing, [], 'these read tools were never called');
    });

    it('lists every baseline entry with a kind and a reason', () => {
      for (const entry of baseline.violations) {
        assert.ok(['write', 'init'].includes(entry.kind), `${entry.tool}: kind is write|init`);
        assert.ok(entry.reason?.trim(), `${entry.tool}: give a reason`);
        if (entry.kind === 'write') {
          assert.ok(
            Array.isArray(entry.tables) &&
              entry.tables.length > 0 &&
              entry.tables.every((table) => typeof table === 'string' && table),
            `${entry.tool}: a 'write' entry lists the tables it writes`,
          );
        } else {
          assert.equal(entry.tables, undefined, `${entry.tool}: only 'write' entries list tables`);
        }
      }
      assert.equal(known.size, baseline.violations.length, 'a tool is listed twice');
    });

    it('introduces no read tool that writes', () => {
      const fresh = realViolations.filter((v) => !known.has(v.tool)).map(describeViolation);
      assert.deepEqual(
        fresh,
        [],
        'these read tools changed persisted state: govern the write (a write permission ' +
          'and a kernel command) or stop writing; do not just add them to the baseline',
      );
      const escalated = realViolations
        .filter((v) => known.get(v.tool)?.kind === 'init' && !isInitializationOnly(v.changes))
        .map(describeViolation);
      assert.deepEqual(
        escalated,
        [],
        "these tools are baselined as first-use 'init' but now change more than empty structure",
      );
      const strayWrites = realViolations.flatMap((v) => {
        const entry = known.get(v.tool);
        if (entry?.kind !== 'write') return [];
        return outsideListedTables(entry, v.changes).map(
          (change) => `${v.tool} [${v.variant}] -> ${describeChange(change, sandbox.root)}`,
        );
      });
      assert.deepEqual(
        strayWrites,
        [],
        "these tools are baselined as 'write' but changed state outside their listed tables",
      );
    });

    it('keeps the baseline honest: a fixed tool must leave it', () => {
      const stale = [...known.keys()].filter((tool) => tools.has(tool) && !violating.has(tool));
      assert.deepEqual(stale, [], 'these no longer change state; remove them from the baseline');
      const staleTables = baseline.violations
        .filter((entry) => entry.kind === 'write' && violating.has(entry.tool))
        .flatMap((entry) => {
          const written = new Set(
            realViolations
              .filter((v) => v.tool === entry.tool)
              .flatMap((v) => v.changes)
              .filter((change) => change.kind === 'rows')
              .map((change) => change.table),
          );
          return entry.tables
            .filter((table) => !written.has(table))
            .map((table) => `${entry.tool}: ${table}`);
        });
      assert.deepEqual(
        staleTables,
        [],
        'these tables are no longer written; remove them from the baseline entry',
      );
      // A tool reclassified as a write is fixed too, but is no longer swept.
      const notRead = only
        ? []
        : [...known.keys()].filter((tool) => !sweep.expectedReadTools.includes(tool));
      assert.deepEqual(
        notRead,
        [],
        'these are no longer read tools; remove them from the baseline',
      );
    });
  });
}

if (unavailable) {
  // Surface why the sweep did not run instead of passing silently.
  describe('read tool sweep prerequisites', () => {
    it('can build the server and seed the store', () => {
      throw unavailable;
    });
  });
}
