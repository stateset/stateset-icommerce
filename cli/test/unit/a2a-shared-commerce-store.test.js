import { afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { A2AStore } from '../../src/a2a/store.js';

let store;
let fixtureDir;

afterEach(() => {
  store?.close();
  store = undefined;
  if (fixtureDir) rmSync(fixtureDir, { recursive: true, force: true });
  fixtureDir = undefined;
});

describe('A2A shared commerce storage', () => {
  it('coexists with native quote/card schemas while sharing kernel escrows', () => {
    fixtureDir = mkdtempSync(join(tmpdir(), 'stateset-a2a-shared-'));
    store = new A2AStore({ dbPath: join(fixtureDir, 'store.db') });

    // Reproduce the three native schema names that overlap the A2A runtime.
    // Quotes and cards intentionally remain native-owned; escrows are the
    // shared aggregate governed by both the JS lifecycle and Rust kernel.
    store.init();
    store.db.exec(`
      DROP TABLE a2a_market_quotes;
      DROP TABLE a2a_runtime_agent_cards;
      DROP TABLE a2a_escrows;
      CREATE TABLE a2a_quotes (
        id TEXT PRIMARY KEY, quote_number TEXT NOT NULL UNIQUE,
        status TEXT NOT NULL, buyer_agent_id TEXT NOT NULL,
        seller_agent_id TEXT NOT NULL, items TEXT NOT NULL,
        subtotal TEXT NOT NULL, tax_amount TEXT NOT NULL,
        shipping_amount TEXT NOT NULL, discount_amount TEXT NOT NULL,
        total TEXT NOT NULL, currency TEXT NOT NULL, valid_until TEXT NOT NULL,
        created_at TEXT NOT NULL, updated_at TEXT NOT NULL
      );
      CREATE TABLE agent_cards (
        id TEXT PRIMARY KEY, name TEXT NOT NULL, wallet_address TEXT NOT NULL UNIQUE,
        public_key TEXT NOT NULL, supported_networks TEXT NOT NULL,
        supported_assets TEXT NOT NULL, trust_level TEXT NOT NULL,
        active INTEGER NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
      );
      CREATE TABLE a2a_escrows (
        id TEXT PRIMARY KEY, status TEXT NOT NULL DEFAULT 'created', quote_id TEXT,
        payment_id TEXT, buyer_address TEXT NOT NULL, seller_address TEXT NOT NULL,
        amount INTEGER NOT NULL, amount_decimal TEXT NOT NULL, asset TEXT NOT NULL,
        network TEXT NOT NULL, release_conditions TEXT NOT NULL DEFAULT '[]',
        funded_at TEXT, released_at TEXT, disputed_at TEXT, dispute_id TEXT,
        expires_at TEXT NOT NULL, auto_release_after TEXT, metadata TEXT,
        created_at TEXT NOT NULL, updated_at TEXT NOT NULL
      );
    `);
    store.close();

    // Reinitializing is the production ordering: native migrations first,
    // followed by the runtime projections on the same database file.
    store.init();
    const quote = store.createQuote({
      buyer_address: '0xbuyer',
      seller_address: '0xseller',
      expires_at: new Date(Date.now() + 60_000).toISOString(),
    });
    const agent = store.registerAgent({ name: 'Seller', wallet_address: '0xseller' });
    const escrow = store.createEscrow({
      buyer_address: '0xbuyer',
      seller_address: '0xseller',
      amount: 100,
      amount_decimal: '1.00',
      expires_at: new Date(Date.now() + 60_000).toISOString(),
    });

    assert.equal(store.db.prepare('SELECT COUNT(*) AS n FROM a2a_quotes').get().n, 0);
    assert.equal(store.db.prepare('SELECT COUNT(*) AS n FROM agent_cards').get().n, 0);
    assert.equal(store.getQuote(quote.id).buyer_address, '0xbuyer');
    assert.equal(store.getAgent(agent.id).wallet_address, '0xseller');
    assert.equal(store.getEscrow(escrow.id).amount_decimal, '1.00');
  });
});
