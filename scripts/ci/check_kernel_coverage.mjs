#!/usr/bin/env node

import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { classifyKernelToolBoundary } from '../../cli/src/kernel-boundary.js';
import { KERNEL_CAPABILITY_BY_TOOL } from '../../cli/src/kernel-tool-execution.js';
import { AGENTIC_RUNTIME_TOOLS } from '../../cli/src/mcp/agentic-runtime-tools.js';
import { ALL_DOMAIN_TOOLS } from '../../cli/src/tools/domain-registry.js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const manifestPath = path.join(root, 'kernel/coverage.json');
const boundaryPath = path.join(root, 'kernel/mutation-boundary.json');
const sqlitePath = path.join(root, 'crates/stateset-db/src/sqlite/kernel_executor.rs');
const postgresPath = path.join(root, 'crates/stateset-db/src/postgres/kernel_executor.rs');
const coreKernelPath = path.join(root, 'crates/stateset-core/src/kernel.rs');
const sharedKernelOutboxPath = path.join(root, 'crates/stateset-db/src/kernel_outbox.rs');
const sqliteOutboxPath = path.join(root, 'crates/stateset-db/src/sqlite/kernel_outbox.rs');
const postgresOutboxPath = path.join(root, 'crates/stateset-db/src/postgres/kernel_outbox.rs');
const testsPath = path.join(root, 'crates/stateset-db/tests/sqlite_kernel_outbox.rs');
const postgresTestsPath = path.join(root, 'crates/stateset-db/tests/postgres_kernel_executor.rs');
const docsPath = path.join(root, 'docs/src/kernel-execution.md');
const embeddedKernelPath = path.join(root, 'crates/stateset-embedded/src/commerce/kernel.rs');
const nodeToolkitPath = path.join(root, 'cli/src/agent-toolkit.js');
const nodeToolkitTestsPath = path.join(root, 'cli/test/unit/agent-toolkit.test.js');
const nodeKernelExecutionPath = path.join(root, 'cli/src/kernel-tool-execution.js');
const mcpPlanExecutorPath = path.join(root, 'cli/src/mcp/plan-step-executor.js');
const mcpServerPath = path.join(root, 'cli/src/mcp-server.js');
const a2aSchemaPath = path.join(root, 'cli/src/a2a/store/schema.js');
const a2aSharedStoreTestPath = path.join(root, 'cli/test/unit/a2a-shared-commerce-store.test.js');
const a2aKernelIntegrationTestPath = path.join(
  root,
  'cli/test/integration/kernel-a2a-shared-store.test.js',
);
const mcpHttpPath = path.join(root, 'cli/bin/stateset-mcp-http.js');
const mcpStdioPath = path.join(root, 'cli/bin/stateset-mcp.js');
const kernelConfigPath = path.join(root, 'cli/src/kernel-config.js');
const kernelConfigTestPath = path.join(root, 'cli/test/unit/kernel-config.test.js');
const pythonToolkitPath = path.join(
  root,
  'bindings/python/python/stateset_embedded/agent_toolkit.py',
);
const requiredGuarantees = [
  'preview',
  'policy',
  'tenant_scope',
  'delegation',
  'idempotency',
  'atomic_receipt',
  'atomic_events',
  'audit_chain',
  'external_checkpoint',
];
const criticalMutationFamilies = [
  'a2a.dispute.evidence.submit',
  'a2a.dispute.file',
  'a2a.dispute.resolve',
  'a2a.escrow.create',
  'a2a.escrow.dispute',
  'a2a.escrow.fund',
  'a2a.escrow.refund',
  'a2a.escrow.release',
  'checkout.commit',
  'inventory.item.create',
  'inventory.reservation.confirm',
  'inventory.reservation.release',
  'inventory.reserve',
  'ledger.post',
  'orders.ship',
  'orders.transition',
  'payments.create',
  'payments.create_refund',
  'products.create',
  'returns.transition',
  'subscriptions.charge',
  'x402.settle',
];

function fail(errors) {
  for (const error of errors) process.stderr.write(`kernel coverage: ${error}\n`);
  process.exitCode = 1;
}

const [
  manifestText,
  boundaryText,
  sqlite,
  postgres,
  coreKernel,
  sharedKernelOutbox,
  sqliteOutbox,
  postgresOutbox,
  tests,
  postgresTests,
  docs,
  embeddedKernel,
  nodeToolkit,
  nodeToolkitTests,
  nodeKernelExecution,
  mcpPlanExecutor,
  mcpServer,
  a2aSchema,
  a2aSharedStoreTest,
  a2aKernelIntegrationTest,
  mcpHttp,
  mcpStdio,
  kernelConfig,
  kernelConfigTest,
  pythonToolkit,
] = await Promise.all([
  readFile(manifestPath, 'utf8'),
  readFile(boundaryPath, 'utf8'),
  readFile(sqlitePath, 'utf8'),
  readFile(postgresPath, 'utf8'),
  readFile(coreKernelPath, 'utf8'),
  readFile(sharedKernelOutboxPath, 'utf8'),
  readFile(sqliteOutboxPath, 'utf8'),
  readFile(postgresOutboxPath, 'utf8'),
  readFile(testsPath, 'utf8'),
  readFile(postgresTestsPath, 'utf8'),
  readFile(docsPath, 'utf8'),
  readFile(embeddedKernelPath, 'utf8'),
  readFile(nodeToolkitPath, 'utf8'),
  readFile(nodeToolkitTestsPath, 'utf8'),
  readFile(nodeKernelExecutionPath, 'utf8'),
  readFile(mcpPlanExecutorPath, 'utf8'),
  readFile(mcpServerPath, 'utf8'),
  readFile(a2aSchemaPath, 'utf8'),
  readFile(a2aSharedStoreTestPath, 'utf8'),
  readFile(a2aKernelIntegrationTestPath, 'utf8'),
  readFile(mcpHttpPath, 'utf8'),
  readFile(mcpStdioPath, 'utf8'),
  readFile(kernelConfigPath, 'utf8'),
  readFile(kernelConfigTestPath, 'utf8'),
  readFile(pythonToolkitPath, 'utf8'),
]);
const manifest = JSON.parse(manifestText);
const boundary = JSON.parse(boundaryText);
const errors = [];

const derivedBoundary = classifyKernelToolBoundary([...ALL_DOMAIN_TOOLS, ...AGENTIC_RUNTIME_TOOLS]);
if (JSON.stringify(boundary) !== JSON.stringify(derivedBoundary)) {
  errors.push(
    'kernel/mutation-boundary.json is stale; run node scripts/ci/generate_kernel_boundary.mjs',
  );
}
for (const [toolName, commandType] of Object.entries(KERNEL_CAPABILITY_BY_TOOL)) {
  const entry = derivedBoundary.entries.find((candidate) => candidate.name === toolName);
  if (!entry) errors.push(`${toolName}: governed tool is absent from the MCP registry`);
  else if (!entry.mutation)
    errors.push(`${toolName}: governed tool is not classified as a mutation`);
  else if (entry.commandType !== commandType || entry.disposition !== 'governed') {
    errors.push(`${toolName}: governed tool classification does not match ${commandType}`);
  }
}
if (
  derivedBoundary.entries.some(
    (entry) =>
      entry.mutation && !['governed', 'governed_composite', 'blocked'].includes(entry.disposition),
  )
) {
  errors.push('one or more mutations are neither governed nor blocked');
}
const planComposite = derivedBoundary.entries.find(
  (entry) => entry.name === 'agentic_execute_plan',
);
if (planComposite?.permission !== 'write' || planComposite?.disposition !== 'governed_composite') {
  errors.push('agentic_execute_plan must be an explicit governed composite mutation');
}

if (manifest.schemaVersion !== 1) errors.push('unsupported schemaVersion');
if (manifest.contractVersion !== '1.0')
  errors.push('contractVersion must match the current kernel wire contract');
if (!Array.isArray(manifest.commands) || manifest.commands.length === 0)
  errors.push('commands must not be empty');

const names = manifest.commands.map((command) => command.name);
if (new Set(names).size !== names.length) errors.push('command names must be unique');
if (JSON.stringify(names) !== JSON.stringify([...names].sort()))
  errors.push('commands must be sorted by name');

for (const command of manifest.commands) {
  if (!['high', 'critical'].includes(command.risk)) errors.push(`${command.name}: invalid risk`);
  if (command.capability !== command.name)
    errors.push(`${command.name}: capability must default to command name`);
  for (const guarantee of requiredGuarantees) {
    if (!command.guarantees?.includes(guarantee))
      errors.push(`${command.name}: missing ${guarantee}`);
  }
  if (!sqlite.includes(`pub fn ${command.sqliteExecutor}`)) {
    errors.push(`${command.name}: SQLite executor ${command.sqliteExecutor} is missing`);
  }
  if (!postgres.includes(`pub async fn ${command.postgresExecutor}`)) {
    errors.push(`${command.name}: PostgreSQL executor ${command.postgresExecutor} is missing`);
  }
  if (!sqlite.includes(`"${command.name}"`) || !postgres.includes(`"${command.name}"`)) {
    errors.push(`${command.name}: command constant is missing from one or both backends`);
  }
  if (!tests.includes(`"${command.name}"`))
    errors.push(`${command.name}: focused kernel test coverage is missing`);
  if (!docs.includes(`\`${command.name}\``) && !docs.includes(command.name)) {
    errors.push(`${command.name}: kernel documentation is missing`);
  }
  if (command.name.startsWith('payments.') && !command.guarantees.includes('exact_money')) {
    errors.push(`${command.name}: payment commands require exact_money`);
  }
  if (
    (command.name.startsWith('inventory.') || command.name === 'orders.ship') &&
    !command.guarantees.includes('exact_quantity')
  ) {
    errors.push(`${command.name}: inventory-affecting commands require exact_quantity`);
  }
}

const declared = new Set(names);
for (const source of [sqlite, postgres]) {
  for (const match of source.matchAll(/^const\s+[A-Z0-9_]+_COMMAND:\s*&str\s*=\s*"([^"]+)";/gm)) {
    if (!declared.has(match[1]))
      errors.push(`${match[1]}: executor command is absent from kernel/coverage.json`);
  }
}

if (!Array.isArray(manifest.requiredNext)) {
  errors.push('requiredNext must be an array');
} else {
  const uncovered = criticalMutationFamilies.filter((name) => !declared.has(name));
  if (JSON.stringify(manifest.requiredNext) !== JSON.stringify(uncovered)) {
    errors.push(
      `requiredNext must exactly track uncovered critical mutation families: ${uncovered.join(', ')}`,
    );
  }
}

for (const token of [
  'execute_kernel_command',
  'inventory.item.create',
  'payments.create',
  'products.create',
  'a2a.dispute.evidence.submit',
  'a2a.dispute.file',
  'a2a.dispute.resolve',
  'a2a.escrow.create',
  'a2a.escrow.dispute',
  'a2a.escrow.fund',
  'a2a.escrow.refund',
  'a2a.escrow.release',
  'subscriptions.charge',
]) {
  if (!embeddedKernel.includes(token))
    errors.push(`embedded kernel dispatcher is missing ${token}`);
}
if (!nodeToolkit.includes('KERNEL_CAPABILITY_BY_TOOL')) {
  errors.push('Node agent toolkit does not capability-scope governed commands');
}
if (!nodeKernelExecution.includes('executeKernelCommand')) {
  errors.push('Node agent toolkit does not invoke the native kernel dispatcher');
}
if (!nodeKernelExecution.includes('kernelConfig.strict !== false')) {
  errors.push('Node governed executor is missing strict raw-write refusal');
}
if (!mcpServer.includes('strictKernelBoundary') || !mcpServer.includes('exposedToolDefs')) {
  errors.push('MCP tool discovery does not hide writes outside the governed catalog');
}
if (!mcpServer.includes('selectStrictKernelToolDefinitions')) {
  errors.push('MCP strict boundary is not using the fail-closed registry classifier');
}
if (
  !nodeToolkitTests.includes('strict kernel mode hides and rejects every unmapped mutation tool')
) {
  errors.push('Node strict kernel exposure test is missing');
}
if (!mcpPlanExecutor.includes('executeGovernedTool')) {
  errors.push('MCP direct and plan execution do not cross the governed command boundary');
}
if (!mcpServer.includes('new A2AStore({ dbPath })')) {
  errors.push('MCP A2A runtime is not using the commerce database');
}
for (const projection of ['a2a_market_quotes', 'a2a_runtime_agent_cards', 'a2a_escrows']) {
  if (!a2aSchema.includes(projection))
    errors.push(`A2A shared-store schema is missing ${projection}`);
  if (!a2aSharedStoreTest.includes(projection))
    errors.push(`A2A shared-store coexistence test is missing ${projection}`);
}
for (const token of [
  'strict: true',
  'a2a_create_escrow',
  'a2a_file_dispute',
  'a2a_submit_evidence',
  'a2a_resolve_dispute',
  'a2a_fund_escrow',
  'a2a_refund_escrow',
  'a2a_release_escrow',
  'audit_hash',
]) {
  if (!a2aKernelIntegrationTest.includes(token))
    errors.push(`A2A governed lifecycle integration test is missing ${token}`);
}
if (!mcpHttp.includes("mkdtempSync(join(tmpdir(), 'stateset-mcp-http-'))")) {
  errors.push('ephemeral MCP HTTP does not provide one file shared by native and A2A connections');
}
for (const source of [mcpStdio, mcpHttp]) {
  for (const token of [
    'kernel-policy',
    'kernel-principal',
    'kernel-store-id',
    'requireForApply: values.apply',
    'kernel,',
  ]) {
    if (!source.includes(token)) errors.push(`an MCP binary is missing trusted ${token} wiring`);
  }
}
for (const token of [
  'STATESET_KERNEL_POLICY',
  'STATESET_KERNEL_PRINCIPAL',
  'STATESET_KERNEL_STORE_ID',
]) {
  if (!kernelConfig.includes(token))
    errors.push(`trusted kernel config loader is missing ${token}`);
  if (!kernelConfigTest.includes(token))
    errors.push(`trusted kernel config test is missing ${token}`);
}
if (!pythonToolkit.includes('execute_kernel_command')) {
  errors.push('Python agent toolkit does not expose the native kernel dispatcher');
}
for (const source of [sqliteOutbox, postgresOutbox]) {
  if (!source.includes('audit_checkpoint'))
    errors.push('a database backend is missing audit checkpoints');
  if (!source.includes('verify_audit_checkpoint')) {
    errors.push('a database backend is missing external checkpoint verification');
  }
}
if (!tests.includes('audit_checkpoints_are_portable_append_stable_and_tamper_evident')) {
  errors.push('SQLite external audit checkpoint eval is missing');
}
if (!postgresTests.includes('verify_audit_checkpoint_async')) {
  errors.push('PostgreSQL external audit checkpoint eval is missing');
}
if (!postgresTests.includes('postgres_audit_checkpoint_rejects_resealed_wrong_sequence')) {
  errors.push('PostgreSQL checkpoint sequence-binding adversarial eval is missing');
}
if (
  !postgresTests.includes('postgres_outbox_leases_retry_dead_letter_redrive_and_ack_are_durable')
) {
  errors.push('PostgreSQL durable outbox recovery parity eval is missing');
}
// The checkpoint must be bound to the POSITION it claims, not merely to a
// matching hash somewhere in the log. It addresses the entry by ordinal
// position rather than by `sequence` value so that a gap left by a
// rolled-back append cannot invalidate an otherwise honest checkpoint; the
// binding is equally strong and `*_rejects_resealed_wrong_sequence` guards it.
if (!sqliteOutbox.includes('ORDER BY sequence LIMIT 1 OFFSET ?')) {
  errors.push('SQLite audit checkpoint is not bound to its claimed position');
}
if (!postgresOutbox.includes('ORDER BY sequence OFFSET $1 LIMIT 1')) {
  errors.push('PostgreSQL audit checkpoint is not bound to its claimed position');
}
if (!coreKernel.includes('"authority": authority')) {
  errors.push('signed authority digest does not bind authority claim metadata');
}
if (!coreKernel.includes('changed.authority.as_mut().expect("authority").expires_at')) {
  errors.push('signed authority validity-window adversarial eval is missing');
}
if (!sharedKernelOutbox.includes('"authority": command.authority')) {
  errors.push('semantic idempotency hash does not bind authority evidence');
}
if (
  !sharedKernelOutbox.includes(
    'semantic_request_hash_binds_authority_but_not_retry_invocation_metadata',
  )
) {
  errors.push('semantic authority/idempotency adversarial eval is missing');
}
for (const source of [sqlite, postgres]) {
  // Every governed op now hashes through `CommandRun::prepare`, which calls
  // the shared `semantic_request_hash` in `src/kernel/run.rs`; an executor
  // that still imports it directly is equally acceptable.
  const usesSharedHash =
    source.includes('use crate::kernel_outbox::semantic_request_hash;') ||
    source.includes('CommandRun::prepare(');
  if (!usesSharedHash) {
    errors.push('a database executor is not using the shared semantic request hash');
  }
  if (/fn semantic_request_hash\s*</.test(source)) {
    errors.push('a database executor has drifted to a backend-local semantic request hash');
  }
}

if (errors.length > 0) fail(errors);
else
  process.stdout.write(`kernel coverage: ${manifest.commands.length} governed commands verified\n`);
