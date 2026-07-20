// Settler-side state. Production runs this against a real database; the demo
// keeps everything in-memory with the same shape so the test surface and HTTP
// endpoints are correct end-to-end.
//
// Durability: set SETTLER_STATE_FILE to make escrows, event logs, and
// settlement receipts survive restarts (ICP-1.0 §8 requires reconstructing
// escrow state from the event log — a settler that forgets open escrows on
// restart cannot honor that). Writes are atomic (tmp + rename). A corrupt
// state file fails startup rather than silently starting empty: for a
// process that tracks who holds money, refusing to run beats amnesia.

import { existsSync, readFileSync, renameSync, writeFileSync } from 'node:fs';

const STATE_FILE = process.env.SETTLER_STATE_FILE ?? null;

const escrows = new Map();      // escrow_id -> { state, intent_id, amount, settler, seq }
const events = new Map();       // escrow_id -> EscrowEvent[]
const settlements = new Map();  // settlement_id -> SettlementReceipt
const observers = new Map();    // escrow_id -> Set<{ write }> — runtime-only, never persisted

function loadPersisted() {
  if (!STATE_FILE || !existsSync(STATE_FILE)) return;
  const doc = JSON.parse(readFileSync(STATE_FILE, 'utf8'));
  for (const [id, escrow] of Object.entries(doc.escrows ?? {})) escrows.set(id, escrow);
  for (const [id, log] of Object.entries(doc.events ?? {})) events.set(id, log);
  for (const [id, receipt] of Object.entries(doc.settlements ?? {})) settlements.set(id, receipt);
}

export function persist() {
  if (!STATE_FILE) return;
  const doc = {
    escrows: Object.fromEntries(escrows),
    events: Object.fromEntries(events),
    settlements: Object.fromEntries(settlements),
  };
  const tmp = `${STATE_FILE}.tmp`;
  writeFileSync(tmp, JSON.stringify(doc, null, 2));
  renameSync(tmp, STATE_FILE);
}

loadPersisted();

export function knownEscrow(id) {
  return escrows.has(id);
}
export function createOrGetEscrow(id, init) {
  let e = escrows.get(id);
  if (!e) {
    e = { ...init, seq: 0 };
    escrows.set(id, e);
    events.set(id, []);
    persist();
  }
  return e;
}
export function updateEscrow(id, patch) {
  const e = escrows.get(id);
  if (!e) throw new Error(`escrow ${id} unknown`);
  Object.assign(e, patch);
  persist();
  return e;
}
export function getEscrow(id) {
  return escrows.get(id);
}
export function appendEvent(escrowId, event) {
  const log = events.get(escrowId);
  if (!log) throw new Error(`escrow ${escrowId} has no event log`);
  log.push(event);
  persist();
  const subs = observers.get(escrowId);
  if (subs) {
    const sseData = `event: escrow.event\ndata: ${JSON.stringify(event)}\n\n`;
    for (const sub of subs) {
      try { sub.write(sseData); } catch (_) { /* sub will drop on close */ }
    }
  }
}
export function getEvents(escrowId) {
  return events.get(escrowId) ?? [];
}
export function recordSettlement(receipt) {
  settlements.set(receipt.settlement_id, receipt);
  persist();
}
export function getSettlement(id) {
  return settlements.get(id);
}
export function addObserver(escrowId, sub) {
  if (!observers.has(escrowId)) observers.set(escrowId, new Set());
  observers.get(escrowId).add(sub);
}
export function removeObserver(escrowId, sub) {
  observers.get(escrowId)?.delete(sub);
}
export function snapshot() {
  return {
    open_escrows: [...escrows.values()].filter(e => e.state !== 'released' && e.state !== 'refunded').length,
    open_escrow_total_units: [...escrows.values()]
      .filter(e => e.state === 'funded' || e.state === 'fulfilled' || e.state === 'disputed')
      .reduce((sum, e) => sum + Number(e.amount?.amount ?? 0), 0)
      .toFixed(2),
    total_escrows: escrows.size,
    total_settlements: settlements.size,
  };
}
