import { createHash } from 'node:crypto';
import { canonicalJson } from './codec.mjs';

// Reference escrow state with optional host-configured transactional storage.
// Native Commerce aggregate adoption and live settlement remain integrations.
//
// Per handler-design.md: the handler is stateless w.r.t. the protocol
// (state lives in the backend or the Settler). What we keep here is just
// enough to demonstrate the lifecycle through HTTP.

let storage = null;
let notifications = null;
const memories = new Map();
export function collection(namespace) {
  const memory = memories.get(namespace) ?? new Map();
  memories.set(namespace, memory);
  const target = () => (storage ? storage.collection(namespace) : memory);
  return {
    get: (key) => structuredClone(target().get(key)),
    set: (key, value) => target().set(key, structuredClone(value)),
    has: (key) => target().has(key),
    values: () => Array.from(target().values(), (value) => structuredClone(value)),
    get size() {
      return target().size;
    },
  };
}

// Configure before importing server.mjs. Never migrate a live process implicitly.
export function configureStorage(store) {
  if (storage || [...memories.values()].some((map) => map.size))
    throw new Error('configure protocol storage before use');
  storage = store;
}
export function isDurable() {
  return storage !== null;
}
export function bindIdentity(identity) {
  storage?.bindIdentity(identity);
}
export function durableReplayGuard(options) {
  return storage.replayGuard(options);
}
export function afterCommit(fn) {
  if (notifications) notifications.push(fn);
  else fn();
}
export function atomic(fn) {
  if (fn.constructor.name === 'AsyncFunction')
    throw new Error('protocol transactions must be synchronous');
  const outer = notifications;
  const pending = [];
  notifications = pending;
  const snapshots = !storage
    ? new Map([...memories].map(([key, value]) => [key, structuredClone(value)]))
    : null;
  let result;
  try {
    result = storage ? storage.atomic(fn) : fn();
    if (result?.then) throw new Error('protocol transactions must be synchronous');
  } catch (error) {
    if (snapshots)
      for (const [key, snapshot] of snapshots) {
        const memory = memories.get(key);
        memory.clear();
        for (const [id, value] of snapshot) memory.set(id, value);
      }
    throw error;
  } finally {
    notifications = outer;
  }
  if (outer) outer.push(...pending);
  else
    for (const notify of pending) {
      try {
        notify();
      } catch {
        /* committed state remains authoritative */
      }
    }
  return result;
}

const intents = collection('intents');
const quotes = collection('quotes');
const escrows = collection('escrows');
const events = collection('events');
const settlements = collection('settlements');
const observers = new Map(); // escrow_id → Set<{ res, write }> SSE subscribers
const seedInventory = new Map([
  ['SKU-100', 100],
  ['WIDGET-001', 47],
  ['WIDGET-002', 0],
  ['WIDGET-003', 12],
  ['GADGET-A', 200],
  ['GADGET-B', 100],
]);
const inventory = collection('inventory');
const reservations = collection('reservations');

function immutableRecord(records, id, value) {
  const prior = records.get(id);
  if (prior !== undefined && canonicalJson(prior) !== canonicalJson(value))
    throw new Error('immutable protocol record conflict');
  if (prior === undefined) records.set(id, value);
}

export function initializeInventory() {
  atomic(() => {
    const metadata = storage?.collection('metadata');
    if (metadata?.get('inventory_initialized')) return;
    for (const [sku, value] of seedInventory) if (!inventory.has(sku)) inventory.set(sku, value);
    metadata?.set('inventory_initialized', true);
  });
}

export function availableInventory(sku) {
  initializeInventory();
  return inventory.get(sku) ?? 0;
}

export function recordIntent(intent, signatureHex, signerPublicKey = null) {
  immutableRecord(intents, intent.intent_id, {
    intent,
    signedAt: intents.get(intent.intent_id)?.signedAt ?? new Date().toISOString(),
    signatureHex,
    signerPublicKey: signerPublicKey ? Buffer.from(signerPublicKey).toString('hex') : null,
  });
}
export function getIntent(intentId) {
  const record = intents.get(intentId);
  return record
    ? {
        ...record,
        signerPublicKey: record.signerPublicKey ? Buffer.from(record.signerPublicKey, 'hex') : null,
      }
    : undefined;
}

export function recordQuote(quote, intentId, signatureHex) {
  immutableRecord(quotes, quote.quote_id, { quote, intentId, signatureHex });
}
export function getQuote(quoteId) {
  return quotes.get(quoteId);
}
export function getQuoteByIntent(intentId) {
  for (const record of quotes.values()) {
    if (record.intentId === intentId) return record;
  }
  return null;
}

export function reserveInventory(escrowId, lines) {
  return atomic(() => reserveInventoryInside(escrowId, lines));
}
function reserveInventoryInside(escrowId, lines) {
  initializeInventory();
  if (!Array.isArray(lines) || lines.length === 0) return null;
  // Aggregate before checking availability: repeated lines consume the same
  // stock pool. Validate the whole request before changing any balance.
  const quantities = new Map();
  for (const line of lines) {
    if (
      !line ||
      typeof line.sku !== 'string' ||
      !Number.isSafeInteger(line.quantity) ||
      line.quantity <= 0
    )
      return null;
    const quantity = (quantities.get(line.sku) ?? 0) + line.quantity;
    if (!Number.isSafeInteger(quantity)) return null;
    quantities.set(line.sku, quantity);
  }
  const normalized = [...quantities].sort(([a], [b]) => a.localeCompare(b));
  const existing = reservations.get(escrowId);
  if (existing) {
    const prior = existing.items.map(({ sku, quantity }) => [sku, quantity]);
    return JSON.stringify(prior) === JSON.stringify(normalized) ? existing : null;
  }
  for (const [sku, quantity] of normalized) {
    const available = inventory.get(sku);
    if (!Number.isSafeInteger(available) || available < quantity) return null;
  }
  const items = normalized.map(([sku, quantity]) => {
    const after = inventory.get(sku) - quantity;
    inventory.set(sku, after);
    return { sku, quantity, available_after: after };
  });
  const reservation = {
    reservation_id: `res_${createHash('sha256').update(escrowId).digest('hex').slice(0, 32)}`,
    escrow_id: escrowId,
    status: 'reserved',
    items,
    expires_at: new Date(Date.now() + 15 * 60 * 1000).toISOString(),
  };
  reservations.set(escrowId, reservation);
  return reservation;
}
export function getInventoryReservation(escrowId) {
  return reservations.get(escrowId) ?? null;
}

export function createEscrow(escrowId, record) {
  if (escrows.has(escrowId)) throw new Error('escrow already exists');
  escrows.set(escrowId, record);
  events.set(escrowId, []);
}
export function updateEscrow(escrowId, patch) {
  const e = escrows.get(escrowId);
  if (!e) return null;
  Object.assign(e, patch);
  escrows.set(escrowId, e);
  return e;
}
export function getEscrow(escrowId) {
  return escrows.get(escrowId);
}

export function appendEscrowEvent(escrowId, event) {
  const log = events.get(escrowId);
  if (!log) throw new Error(`escrow ${escrowId} not found`);
  if (event.seq !== log.length) throw new Error('escrow event sequence mismatch');
  log.push(event);
  events.set(escrowId, log);
  updateEscrow(escrowId, { seq: event.seq });
  // Fan out to SSE subscribers.
  afterCommit(() => {
    const subs = observers.get(escrowId);
    if (subs) {
      const data = `event: escrow.event\ndata: ${JSON.stringify(event)}\n\n`;
      for (const sub of subs) {
        try {
          sub.write(data);
        } catch (_) {
          /* sub will be removed on close */
        }
      }
    }
  });
}
export function getEscrowEvents(escrowId) {
  return events.get(escrowId) ?? [];
}

export function recordSettlement(receipt) {
  immutableRecord(settlements, receipt.settlement_id, receipt);
}
export function getSettlement(settlementId) {
  return settlements.get(settlementId);
}

export function addObserver(escrowId, sub) {
  if (!observers.has(escrowId)) observers.set(escrowId, new Set());
  observers.get(escrowId).add(sub);
}
export function removeObserver(escrowId, sub) {
  observers.get(escrowId)?.delete(sub);
}

// Telemetry helpers
export function counts() {
  return {
    intents: intents.size,
    quotes: quotes.size,
    escrows: escrows.size,
    settlements: settlements.size,
    reservations: reservations.size,
  };
}
