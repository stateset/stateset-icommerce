/**
 * The oracle: after the swarm stops, read the database directly (read-only)
 * and check what the engine promised against what it stored and what every
 * caller was told.
 *
 *   oversold          - stock: reservations never exceed initial stock,
 *                       reject_if_insufficient orders never backorder, and
 *                       the balance row agrees with the reservations.
 *   overspent         - budgets: committed <= limit, and committed equals the
 *                       sum of succeeded checkouts sealed against the budget.
 *   receiptMismatches - every applied command has exactly one sealed receipt,
 *                       every sealed receipt reached a caller, and every
 *                       mutation (customer, cart, line, order) has a receipt.
 *   duplicateApplies  - one idempotency key, at most one sealed outcome; one
 *                       cart, at most one order; replays return the original.
 *   chainValid        - the receipt audit chain recomputes. The Node binding
 *                       on master exposes no verify API (the Rust
 *                       `Commerce::kernel_audit().verify_chain()` is not
 *                       bound), so this recomputes the same sha256-jcs-v1
 *                       links independently.
 *   crashedActors     - no actor's journey threw. A journey that throws
 *                       before its first write leaves a consistent store, so
 *                       no store invariant can catch it; it fails the verdict
 *                       on its own, and its exceptions travel with the result.
 */
import { fork } from 'node:child_process';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';

import { canonicalJson } from '../../bindings/node/canonical-json.mjs';
import { formatDecimal, parseDecimal, sumDecimals } from './decimal.mjs';
import { openReader } from './engine.mjs';
import { EXPECTED_DENIAL, SKU, principalCount, principalPlan } from './plan.mjs';

const sha256 = (text) => createHash('sha256').update(text).digest('hex');

/**
 * Run the oracle in a child process that never loads the engine.
 *
 * The oracle's reader (better-sqlite3 or node:sqlite) is a second copy of
 * SQLite. Two SQLite copies must not open one file from one process: POSIX
 * advisory locks belong to the process, so neither copy sees the other's
 * locks (https://www.sqlite.org/howtocorrupt.html, section 2.2.1). And the
 * binding's `close()` resolves before the engine has let go of the file -- it
 * hands the engine's drop (pool shutdown, final WAL checkpoint) to a detached
 * thread -- so reading in-process right after `close()` raced that shutdown:
 * the reader believed it was the first connection, reset the WAL index the
 * closing engine still had mapped, and the process died with SIGBUS or read
 * "database disk image is malformed". A separate process gets real locks.
 */
export function runOracleIsolated(input) {
  return new Promise((resolve, reject) => {
    const child = fork(fileURLToPath(new URL('./oracle-process.mjs', import.meta.url)), [], {
      stdio: ['ignore', 'inherit', 'inherit', 'ipc'],
      serialization: 'advanced',
    });
    let settled = false;
    child.once('message', (message) => {
      settled = true;
      if (message.ok) resolve(message.result);
      else reject(new Error(`oracle failed: ${message.error}`));
    });
    child.once('exit', (code, signal) => {
      if (!settled)
        reject(new Error(`oracle process exited (${signal ?? code}) without a verdict`));
    });
    child.send(input);
  });
}

export async function runOracle({ dbPath, config, records, budgetStatuses }) {
  const db = await openReader(dbPath);
  try {
    const auditRows = db.all(
      `SELECT sequence, receipt_id, idempotency_key, request_hash, previous_audit_hash, audit_hash, receipt
         FROM kernel_receipt_audit_log ORDER BY sequence`,
    );
    const audit = auditRows.map((row) => ({ ...row, parsed: JSON.parse(row.receipt) }));
    const materialized = db.all(
      'SELECT idempotency_key, request_hash, status, receipt FROM kernel_receipts',
    );
    const observed = records.flatMap((record) => record.observed);

    const chain = verifyChain(audit, materialized);
    const receipts = checkReceipts(db, audit, observed);
    const idempotency = checkIdempotency(db, audit, observed, records);
    const stock = checkStock(db, config, records);
    const budgets = checkBudgets(config, audit, records, budgetStatuses, db);
    const expectations = checkExpectations(records, observed);
    const crashes = checkCrashes(records);

    const invariants = {
      oversold: stock.violations.length > 0,
      overspent: budgets.overspentCount,
      budgetLedgerMismatches: budgets.ledgerMismatchCount,
      receiptMismatches: receipts.issueCount,
      duplicateApplies: idempotency.issueCount,
      chainValid: chain.valid,
      expectationFailures: expectations.failureCount,
      crashedActors: crashes.count,
    };
    invariants.passed =
      !invariants.oversold &&
      invariants.overspent === 0 &&
      invariants.budgetLedgerMismatches === 0 &&
      invariants.receiptMismatches === 0 &&
      invariants.duplicateApplies === 0 &&
      invariants.chainValid &&
      invariants.expectationFailures === 0 &&
      invariants.crashedActors === 0;

    return {
      invariants,
      chain,
      receipts,
      idempotency,
      stock,
      budgets,
      expectations,
      crashes,
      reader: db.driver,
    };
  } finally {
    db.close();
  }
}

function verifyChain(audit, materialized) {
  let previous = null;
  let firstInvalidSequence = null;
  for (const row of audit) {
    const computed = sha256(
      canonicalJson({
        algorithm: 'sha256-jcs-v1',
        previous_audit_hash: previous,
        request_hash: row.request_hash,
        receipt: row.parsed,
      }),
    );
    if (row.previous_audit_hash !== previous || row.audit_hash !== computed) {
      firstInvalidSequence = row.sequence;
      break;
    }
    previous = row.audit_hash;
  }
  // A materialized receipt must be byte-for-byte the sealed entry it claims.
  const byHash = new Map(audit.map((row) => [row.audit_hash, row]));
  const materializedMismatches = [];
  for (const row of materialized) {
    const sealed = JSON.parse(row.receipt);
    const entry = byHash.get(sealed.audit_hash);
    sealed.audit_hash = null;
    if (
      !entry ||
      entry.request_hash !== row.request_hash ||
      canonicalJson(entry.parsed) !== canonicalJson(sealed)
    ) {
      materializedMismatches.push(row.idempotency_key);
    }
  }
  return {
    verifier: 'js-recompute:sha256-jcs-v1',
    valid: firstInvalidSequence === null && materializedMismatches.length === 0,
    entries: audit.length,
    headHash: previous,
    firstInvalidSequence,
    materializedRows: materialized.length,
    materializedMismatches: materializedMismatches.slice(0, 20),
  };
}

function checkReceipts(db, audit, observed) {
  const issues = [];
  const byReceiptId = new Map();
  for (const row of audit) {
    if (!byReceiptId.has(row.receipt_id)) byReceiptId.set(row.receipt_id, []);
    byReceiptId.get(row.receipt_id).push(row);
  }
  const seen = new Set();
  for (const entry of observed) {
    if (entry.auditHash === null || entry.auditHash === undefined) {
      // The only receipt the kernel hands back unsealed is the idempotency
      // conflict: nothing executed, so nothing is chained.
      if (entry.errorCode !== 'kernel.idempotency_conflict') {
        issues.push({
          kind: 'unsealed_receipt',
          key: entry.key,
          code: entry.errorCode ?? entry.status,
        });
      }
      continue;
    }
    if (seen.has(entry.receiptId)) continue;
    seen.add(entry.receiptId);
    const rows = byReceiptId.get(entry.receiptId) ?? [];
    if (rows.length !== 1 || rows[0].audit_hash !== entry.auditHash) {
      issues.push({
        kind: 'receipt_not_sealed_once',
        key: entry.key,
        receiptId: entry.receiptId,
        sealed: rows.length,
      });
    }
  }
  const unobserved = audit.filter((row) => !seen.has(row.receipt_id));
  for (const row of unobserved) {
    issues.push({
      kind: 'sealed_receipt_never_returned',
      key: row.idempotency_key,
      receiptId: row.receipt_id,
    });
  }

  // Mutation <-> receipt correspondence, in both directions.
  const succeeded = (type) =>
    audit.filter((row) => row.parsed.command_type === type && row.parsed.status === 'succeeded');
  const counts = {
    customers: [
      db.get('SELECT COUNT(*) AS n FROM customers').n,
      succeeded('customers.create').length,
    ],
    carts: [db.get('SELECT COUNT(*) AS n FROM carts').n, succeeded('carts.create').length],
    cartItems: [
      db.get('SELECT COUNT(*) AS n FROM cart_items').n,
      succeeded('carts.item.add').length,
    ],
    orders: [db.get('SELECT COUNT(*) AS n FROM orders').n, succeeded('checkout.commit').length],
  };
  for (const [table, [rows, receipts]] of Object.entries(counts)) {
    if (rows !== receipts)
      issues.push({ kind: 'mutation_receipt_count', table, rows, succeededReceipts: receipts });
  }
  const checkoutCarts = new Set(succeeded('checkout.commit').map((row) => row.parsed.aggregate_id));
  const checkoutOrders = new Set(
    succeeded('checkout.commit').map((row) => row.parsed.result?.order_id),
  );
  for (const order of db.all('SELECT id, cart_id FROM orders')) {
    if (!checkoutCarts.has(order.cart_id) || !checkoutOrders.has(order.id)) {
      issues.push({
        kind: 'order_without_checkout_receipt',
        orderId: order.id,
        cartId: order.cart_id,
      });
    }
  }
  return {
    observedReceipts: observed.length,
    distinctObserved: seen.size,
    sealed: audit.length,
    mutationCounts: Object.fromEntries(
      Object.entries(counts).map(([k, [rows, receipts]]) => [k, { rows, receipts }]),
    ),
    issues: issues.slice(0, 50),
    issueCount: issues.length,
  };
}

function checkIdempotency(db, audit, observed, records) {
  const issues = [];
  const sealsByKey = new Map();
  for (const row of audit)
    sealsByKey.set(row.idempotency_key, (sealsByKey.get(row.idempotency_key) ?? 0) + 1);
  for (const [key, seals] of sealsByKey) {
    // The swarm never previews, so a key is sealed exactly once.
    if (seals > 1) issues.push({ kind: 'key_sealed_more_than_once', key, seals });
  }
  for (const row of db.all(
    'SELECT cart_id, COUNT(*) AS n FROM orders GROUP BY cart_id HAVING COUNT(*) > 1',
  )) {
    issues.push({ kind: 'cart_ordered_more_than_once', cartId: row.cart_id, orders: row.n });
  }
  const receiptsByKey = new Map();
  for (const entry of observed) {
    if (entry.role === 'conflict') {
      if (entry.status === 'succeeded')
        issues.push({ kind: 'conflicting_reuse_applied', key: entry.key });
      continue;
    }
    if (!receiptsByKey.has(entry.key)) receiptsByKey.set(entry.key, new Set());
    receiptsByKey.get(entry.key).add(entry.receiptId);
  }
  for (const [key, ids] of receiptsByKey) {
    if (ids.size > 1)
      issues.push({ kind: 'key_returned_different_receipts', key, receipts: ids.size });
  }
  let replays = 0;
  for (const record of records) {
    for (const purchase of record.purchases) {
      if (!purchase.replay) continue;
      replays += 1;
      if (!purchase.replay.matched) {
        issues.push({
          kind: 'replay_mismatch',
          actor: record.index,
          purchase: purchase.index,
          mode: purchase.replay.mode,
        });
      }
    }
  }
  return { keys: sealsByKey.size, replays, issues: issues.slice(0, 50), issueCount: issues.length };
}

function checkStock(db, config, records) {
  const violations = [];
  const item = db.get('SELECT id FROM inventory_items WHERE sku = ?', SKU);
  const balances = db.all(
    'SELECT quantity_on_hand, quantity_allocated, quantity_available FROM inventory_balances WHERE item_id = ?',
    item.id,
  );
  const onHand = sumDecimals(balances.map((row) => row.quantity_on_hand));
  const allocated = sumDecimals(balances.map((row) => row.quantity_allocated));
  const available = sumDecimals(balances.map((row) => row.quantity_available));
  const reservations = db.all(
    'SELECT quantity, status, reference_type FROM inventory_reservations WHERE item_id = ?',
    item.id,
  );
  const reservationsByStatus = {};
  for (const row of reservations) {
    reservationsByStatus[row.status] = formatDecimal(
      parseDecimal(reservationsByStatus[row.status] ?? '0') + parseDecimal(row.quantity),
    );
  }
  const reserved = sumDecimals(
    reservations.filter((row) => row.status === 'pending').map((row) => row.quantity),
  );
  const ordered = sumDecimals(
    db
      .all('SELECT quantity FROM order_items WHERE sku = ?', SKU)
      .map((row) => String(row.quantity)),
  );
  const backorders = db.all(
    `SELECT b.quantity_ordered, o.cart_id FROM backorders b LEFT JOIN orders o ON o.id = b.order_id WHERE b.sku = ?`,
    SKU,
  );
  const backordered = sumDecimals(backorders.map((row) => row.quantity_ordered));
  const initial = parseDecimal(String(config.stock));

  const policyByCart = new Map();
  let unitsFromReceipts = 0n;
  for (const record of records) {
    for (const purchase of record.purchases) {
      if (purchase.cartId) policyByCart.set(purchase.cartId, purchase.stockPolicy);
      if (purchase.result === 'ordered')
        unitsFromReceipts += parseDecimal(String(purchase.quantity));
    }
  }
  const strictBackorders = backorders.filter(
    (row) => policyByCart.get(row.cart_id) === 'reject_if_insufficient',
  );

  const check = (ok, violation) => {
    if (!ok) violations.push(violation);
  };
  check(
    reserved <= initial,
    `reserved ${formatDecimal(reserved)} exceeds initial stock ${config.stock}`,
  );
  check(available >= 0n, `quantity_available is negative (${formatDecimal(available)})`);
  check(
    strictBackorders.length === 0,
    `${strictBackorders.length} reject_if_insufficient orders were backordered`,
  );
  check(
    onHand === initial,
    `quantity_on_hand ${formatDecimal(onHand)} != initial ${config.stock} (nothing ships)`,
  );
  check(
    allocated === reserved,
    `quantity_allocated ${formatDecimal(allocated)} != open reservations ${formatDecimal(reserved)}`,
  );
  check(
    available === onHand - allocated,
    `quantity_available ${formatDecimal(available)} != on_hand - allocated`,
  );
  check(
    ordered === reserved + backordered,
    `ordered ${formatDecimal(ordered)} != reserved ${formatDecimal(reserved)} + backordered ${formatDecimal(backordered)}`,
  );
  check(
    ordered === unitsFromReceipts,
    `ordered ${formatDecimal(ordered)} != units in succeeded checkout receipts ${formatDecimal(unitsFromReceipts)}`,
  );
  return {
    initial: config.stock,
    onHand: formatDecimal(onHand),
    allocated: formatDecimal(allocated),
    available: formatDecimal(available),
    reserved: formatDecimal(reserved),
    ordered: formatDecimal(ordered),
    backordered: formatDecimal(backordered),
    reservationsByStatus,
    violations,
  };
}

function checkBudgets(config, audit, records, budgetStatuses, db) {
  const overspent = [];
  const ledgerMismatches = [];
  const sealedSpend = new Map();
  for (const row of audit) {
    const receipt = row.parsed;
    if (receipt.command_type !== 'checkout.commit' || receipt.status !== 'succeeded') continue;
    const budgetId = receipt.economic_context?.commitment?.budget_id;
    sealedSpend.set(
      budgetId,
      (sealedSpend.get(budgetId) ?? 0n) + parseDecimal(receipt.result?.total_charged),
    );
  }
  const toldSpend = new Map();
  for (const record of records) {
    const { budgetId } = principalPlan(record.principalIndex, config);
    toldSpend.set(budgetId, (toldSpend.get(budgetId) ?? 0n) + sumDecimals(record.spent));
  }
  const rows = new Map(
    db
      .all('SELECT budget_id, limit_amount, committed_amount FROM kernel_economic_budgets')
      .map((row) => [row.budget_id, row]),
  );
  let exhausted = 0;
  let committedTotal = 0n;
  for (let index = 0; index < principalCount(config); index += 1) {
    const { budgetId } = principalPlan(index, config);
    const status = budgetStatuses[budgetId];
    const row = rows.get(budgetId);
    if (!status || !row) {
      ledgerMismatches.push({ budgetId, problem: 'budget missing' });
      continue;
    }
    const limit = parseDecimal(status.budget.limit.amount);
    const committed = parseDecimal(status.committed.amount);
    const available = parseDecimal(status.available.amount);
    committedTotal += committed;
    if (available < parseDecimal(config.price)) exhausted += 1;
    if (committed > limit || available < 0n)
      overspent.push({
        budgetId,
        limit: formatDecimal(limit),
        committed: formatDecimal(committed),
      });
    const sealed = sealedSpend.get(budgetId) ?? 0n;
    const told = toldSpend.get(budgetId) ?? 0n;
    if (
      committed !== parseDecimal(row.committed_amount) ||
      limit !== parseDecimal(row.limit_amount) ||
      committed !== sealed ||
      committed !== told ||
      committed + available !== limit
    ) {
      ledgerMismatches.push({
        budgetId,
        committed: formatDecimal(committed),
        row: row.committed_amount,
        sealedCheckouts: formatDecimal(sealed),
        toldCallers: formatDecimal(told),
        available: formatDecimal(available),
      });
    }
  }
  return {
    budgets: principalCount(config),
    limitEach: config.budget,
    committedTotal: formatDecimal(committedTotal),
    exhausted,
    overspentCount: overspent.length,
    ledgerMismatchCount: ledgerMismatches.length,
    overspent: overspent.slice(0, 50),
    ledgerMismatches: ledgerMismatches.slice(0, 50),
  };
}

function checkExpectations(records, observed) {
  const failures = [];
  const denialByActor = new Map();
  for (const entry of observed) {
    if (entry.errorCode === EXPECTED_DENIAL.errorCode) denialByActor.set(entry.actor, entry);
  }
  let deniedActors = 0;
  let deniedObserved = 0;
  let conflictAttempts = 0;
  let conflictsRejected = 0;
  for (const record of records) {
    const denial = denialByActor.get(record.index);
    const reachedCheckout = record.purchases.some(
      (purchase) => purchase.result && !purchase.result.startsWith('failed:'),
    );
    if (record.expectDenied) {
      deniedActors += 1;
      if (denial) deniedObserved += 1;
      if (!denial && reachedCheckout)
        failures.push({ kind: 'missing_denial', actor: record.index });
      if (denial && !denial.reasonCodes.includes(EXPECTED_DENIAL.reasonCode)) {
        failures.push({
          kind: 'wrong_denial_reason',
          actor: record.index,
          reasons: denial.reasonCodes,
        });
      }
    } else if (denial) {
      failures.push({
        kind: 'unexpected_denial',
        actor: record.index,
        reasons: denial.reasonCodes,
      });
    }
    for (const purchase of record.purchases) {
      if (!purchase.conflictCode) continue;
      conflictAttempts += 1;
      if (purchase.conflictCode === 'kernel.idempotency_conflict') conflictsRejected += 1;
      else
        failures.push({
          kind: 'conflict_not_rejected',
          actor: record.index,
          code: purchase.conflictCode,
        });
    }
  }
  return {
    deniedActors,
    deniedObserved,
    conflictAttempts,
    conflictsRejected,
    failureCount: failures.length,
    failures: failures.slice(0, 50),
  };
}

/**
 * Actors whose journey threw (the scheduler marks them `crashed`), with every
 * exception they recorded: the journey's own, plus any thrown engine calls
 * before it.
 */
function checkCrashes(records) {
  const crashed = records.filter((record) => record.outcome === 'crashed');
  return {
    count: crashed.length,
    actors: crashed.slice(0, 50).map((record) => ({ actor: record.index, thrown: record.thrown })),
  };
}
