// In-memory escrow state. Production handlers would back this with the
// engine's existing storage; we keep it ephemeral for the demo.
//
// Per handler-design.md: the handler is stateless w.r.t. the protocol
// (state lives in the backend or the Settler). What we keep here is just
// enough to demonstrate the lifecycle through HTTP.

const intents = new Map();    // intent_id → { intent, signedAt, signatureHex }
const quotes = new Map();     // quote_id → { quote, intentId, signatureHex }
const escrows = new Map();    // escrow_id → { state, intent_id, quote_id, ... }
const events = new Map();     // escrow_id → EscrowEvent[] (append-only)
const settlements = new Map();// settlement_id → SettlementReceipt
const observers = new Map();  // escrow_id → Set<{ res, write }> SSE subscribers
const inventory = new Map([
  ['SKU-100', 100],
  ['WIDGET-001', 47],
  ['WIDGET-002', 0],
  ['WIDGET-003', 12],
  ['GADGET-A', 200],
  ['GADGET-B', 100],
]);
const reservations = new Map(); // escrow_id → immutable reservation projection

export function recordIntent(intent, signatureHex) {
  intents.set(intent.intent_id, { intent, signedAt: new Date().toISOString(), signatureHex });
}
export function getIntent(intentId) { return intents.get(intentId); }

export function recordQuote(quote, intentId, signatureHex) {
  quotes.set(quote.quote_id, { quote, intentId, signatureHex });
}
export function getQuote(quoteId) { return quotes.get(quoteId); }
export function getQuoteByIntent(intentId) {
  for (const record of quotes.values()) {
    if (record.intentId === intentId) return record;
  }
  return null;
}

export function reserveInventory(escrowId, lines) {
  if (reservations.has(escrowId)) return reservations.get(escrowId);
  for (const line of lines) {
    const available = inventory.get(line.sku);
    if (!Number.isSafeInteger(available) || !Number.isSafeInteger(line.quantity)
        || line.quantity <= 0 || available < line.quantity) {
      return null;
    }
  }
  const items = lines.map((line) => {
    const before = inventory.get(line.sku);
    inventory.set(line.sku, before - line.quantity);
    return { sku: line.sku, quantity: line.quantity, available_after: before - line.quantity };
  });
  const reservation = {
    reservation_id: `res_${escrowId.slice(2, 18)}`,
    escrow_id: escrowId,
    status: 'reserved',
    items,
    expires_at: new Date(Date.now() + 15 * 60 * 1000).toISOString(),
  };
  reservations.set(escrowId, reservation);
  return reservation;
}
export function getInventoryReservation(escrowId) { return reservations.get(escrowId) ?? null; }

export function createEscrow(escrowId, record) {
  escrows.set(escrowId, record);
  events.set(escrowId, []);
}
export function updateEscrow(escrowId, patch) {
  const e = escrows.get(escrowId);
  if (!e) return null;
  Object.assign(e, patch);
  return e;
}
export function getEscrow(escrowId) { return escrows.get(escrowId); }

export function appendEscrowEvent(escrowId, event) {
  const log = events.get(escrowId);
  if (!log) throw new Error(`escrow ${escrowId} not found`);
  log.push(event);
  // Fan out to SSE subscribers.
  const subs = observers.get(escrowId);
  if (subs) {
    const data = `event: escrow.event\ndata: ${JSON.stringify(event)}\n\n`;
    for (const sub of subs) {
      try { sub.write(data); } catch (_) { /* sub will be removed on close */ }
    }
  }
}
export function getEscrowEvents(escrowId) {
  return events.get(escrowId) ?? [];
}

export function recordSettlement(receipt) {
  settlements.set(receipt.settlement_id, receipt);
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
