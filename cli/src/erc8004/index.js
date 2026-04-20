/**
 * ERC-8004 Identity Registry helpers (SQLite-backed)
 */

import { createRequire } from 'node:module';
import { randomUUID } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const IDENTITY_SCHEMA = `
CREATE TABLE IF NOT EXISTS agent_identities (
  id TEXT PRIMARY KEY,
  agent_registry TEXT NOT NULL,
  agent_id TEXT NOT NULL,
  agent_uri TEXT NOT NULL,
  agent_wallet TEXT,
  owner_address TEXT,
  agent_card_id TEXT,
  registration TEXT,
  registration_hash TEXT,
  wallet_proof_type TEXT,
  wallet_proof TEXT,
  wallet_proof_chain_id INTEGER,
  wallet_proof_deadline TEXT,
  active INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(agent_registry, agent_id)
);

CREATE INDEX IF NOT EXISTS idx_agent_identities_registry
  ON agent_identities(agent_registry);
CREATE INDEX IF NOT EXISTS idx_agent_identities_wallet
  ON agent_identities(agent_wallet);
CREATE INDEX IF NOT EXISTS idx_agent_identities_owner
  ON agent_identities(owner_address);
CREATE INDEX IF NOT EXISTS idx_agent_identities_active
  ON agent_identities(active);
`;

const PROOF_TYPES = new Set(['eip712', 'erc1271']);
const require = createRequire(import.meta.url);
const FALLBACK_IDENTITY_DATABASES = new Map();
let cachedDatabaseCtor;

function normalizeProofType(value) {
  if (!value) return null;
  const normalized = String(value).toLowerCase();
  if (!PROOF_TYPES.has(normalized)) {
    throw new Error(`Invalid proof type: ${value}. Expected eip712 or erc1271.`);
  }
  return normalized;
}

function loadDatabaseCtor() {
  if (cachedDatabaseCtor !== undefined) {
    return cachedDatabaseCtor;
  }

  try {
    const mod = require('better-sqlite3');
    cachedDatabaseCtor = mod.default || mod;
  } catch (error) {
    if (error?.code !== 'ERR_DLOPEN_FAILED' && error?.code !== 'MODULE_NOT_FOUND') {
      throw error;
    }
    cachedDatabaseCtor = null;
  }

  return cachedDatabaseCtor;
}

function ensureDbFile(dbPath) {
  if (!dbPath) {
    throw new Error('dbPath is required');
  }
  if (dbPath === ':memory:') return;
  fs.mkdirSync(path.dirname(dbPath), { recursive: true });
  const fd = fs.openSync(dbPath, 'a');
  fs.closeSync(fd);
}

function getFallbackIdentityPath(dbPath) {
  return dbPath === ':memory:' ? ':memory:' : `${dbPath}.fallback.json`;
}

function persistFallbackState(state) {
  if (!state?.storagePath || state.storagePath === ':memory:') {
    return;
  }

  fs.mkdirSync(path.dirname(state.storagePath), { recursive: true });
  const tmpPath = `${state.storagePath}.tmp`;
  fs.writeFileSync(
    tmpPath,
    JSON.stringify(
      {
        rows: Array.from(state.rows.values()),
      },
      null,
      2,
    ),
  );
  fs.renameSync(tmpPath, state.storagePath);
}

function getFallbackState(dbPath) {
  const storagePath = getFallbackIdentityPath(dbPath);
  if (storagePath === ':memory:') {
    return { rows: new Map(), storagePath };
  }

  let state = FALLBACK_IDENTITY_DATABASES.get(storagePath);
  if (!state) {
    const rows = new Map();
    if (fs.existsSync(storagePath)) {
      try {
        const raw = fs.readFileSync(storagePath, 'utf8').trim();
        if (raw) {
          const parsed = JSON.parse(raw);
          for (const row of parsed?.rows || []) {
            if (!row?.agent_registry || !row?.agent_id) continue;
            rows.set(identityKey(row.agent_registry, row.agent_id), row);
          }
        }
      } catch (error) {
        console.warn(`[erc8004] Failed to read fallback store ${storagePath}: ${error.message}`);
      }
    }
    state = { rows, storagePath };
    FALLBACK_IDENTITY_DATABASES.set(storagePath, state);
  }
  return state;
}

function identityKey(agentRegistry, agentId) {
  return `${agentRegistry}\u0000${agentId}`;
}

function openStore(dbPath, { databaseCtor } = {}) {
  ensureDbFile(dbPath);
  const Database = databaseCtor === undefined ? loadDatabaseCtor() : databaseCtor;
  if (!Database) {
    const state = getFallbackState(dbPath);
    if (dbPath !== ':memory:') {
      console.warn(
        `[erc8004] better-sqlite3 unavailable; using durable JSON fallback at ${state.storagePath}`,
      );
    }
    return {
      db: null,
      state,
      close() {
        persistFallbackState(state);
      },
    };
  }

  try {
    const db = new Database(dbPath);
    db.pragma('journal_mode = WAL');
    db.exec(IDENTITY_SCHEMA);
    return {
      db,
      state: null,
      close() {
        db.close();
      },
    };
  } catch (error) {
    if (error?.code !== 'ERR_DLOPEN_FAILED') {
      throw error;
    }
    const state = getFallbackState(dbPath);
    if (dbPath !== ':memory:') {
      console.warn(
        `[erc8004] ${error.message || 'native module load failure'}; using durable JSON fallback at ${state.storagePath}`,
      );
    }
    return {
      db: null,
      state,
      close() {
        persistFallbackState(state);
      },
    };
  }
}

function selectIdentity(state, agentRegistry, agentId) {
  return state.rows.get(identityKey(agentRegistry, agentId)) || null;
}

function selectIdentityByWallet(state, wallet) {
  for (const row of state.rows.values()) {
    if (row.agent_wallet === wallet) {
      return row;
    }
  }
  return null;
}

function sortIdentitiesByUpdatedAt(rows) {
  return [...rows].sort(
    (a, b) => b.updated_at.localeCompare(a.updated_at) || b.created_at.localeCompare(a.created_at),
  );
}

function mapIdentity(row) {
  if (!row) return null;
  return {
    ...row,
    active: Boolean(row.active),
  };
}

export function registerIdentity(dbPath, input, options = {}) {
  const store = openStore(dbPath, options);
  const now = new Date().toISOString();
  const active = input.active === undefined ? 1 : input.active ? 1 : 0;
  const proofType = normalizeProofType(input.walletProofType);
  const existing = store.state
    ? selectIdentity(store.state, input.agentRegistry, input.agentId)
    : null;
  const id = existing?.id || input.id || randomUUID();
  const createdAt = existing?.created_at || now;

  if (store.state) {
    const row = {
      id,
      agent_registry: input.agentRegistry,
      agent_id: input.agentId,
      agent_uri: input.agentUri,
      agent_wallet: input.agentWallet || null,
      owner_address: input.ownerAddress || null,
      agent_card_id: input.agentCardId || null,
      registration: input.registration || null,
      registration_hash: input.registrationHash || null,
      wallet_proof_type: proofType,
      wallet_proof: input.walletProof || null,
      wallet_proof_chain_id: input.walletProofChainId || null,
      wallet_proof_deadline: input.walletProofDeadline || null,
      active,
      created_at: createdAt,
      updated_at: now,
    };
    store.state.rows.set(identityKey(input.agentRegistry, input.agentId), row);
    persistFallbackState(store.state);
    store.close();
    return mapIdentity(row);
  }

  const stmt = store.db.prepare(`
    INSERT INTO agent_identities (
      id, agent_registry, agent_id, agent_uri, agent_wallet, owner_address,
      agent_card_id, registration, registration_hash, wallet_proof_type,
      wallet_proof, wallet_proof_chain_id, wallet_proof_deadline, active,
      created_at, updated_at
    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    ON CONFLICT(agent_registry, agent_id) DO UPDATE SET
      agent_uri = excluded.agent_uri,
      agent_wallet = excluded.agent_wallet,
      owner_address = excluded.owner_address,
      agent_card_id = excluded.agent_card_id,
      registration = excluded.registration,
      registration_hash = excluded.registration_hash,
      wallet_proof_type = excluded.wallet_proof_type,
      wallet_proof = excluded.wallet_proof,
      wallet_proof_chain_id = excluded.wallet_proof_chain_id,
      wallet_proof_deadline = excluded.wallet_proof_deadline,
      active = excluded.active,
      updated_at = excluded.updated_at
  `);

  stmt.run(
    id,
    input.agentRegistry,
    input.agentId,
    input.agentUri,
    input.agentWallet || null,
    input.ownerAddress || null,
    input.agentCardId || null,
    input.registration || null,
    input.registrationHash || null,
    proofType,
    input.walletProof || null,
    input.walletProofChainId || null,
    input.walletProofDeadline || null,
    active,
    createdAt,
    now,
  );

  const record = store.db
    .prepare('SELECT * FROM agent_identities WHERE agent_registry = ? AND agent_id = ?')
    .get(input.agentRegistry, input.agentId);
  store.close();
  return mapIdentity(record);
}

export function setAgentWallet(dbPath, input, options = {}) {
  const store = openStore(dbPath, options);
  const now = new Date().toISOString();
  const proofType = normalizeProofType(input.walletProofType);
  if (store.state) {
    const existing = selectIdentity(store.state, input.agentRegistry, input.agentId);
    if (!existing) {
      store.close();
      throw new Error(`Agent identity not found for ${input.agentRegistry}:${input.agentId}`);
    }

    const row = {
      ...existing,
      agent_wallet: input.agentWallet,
      wallet_proof_type: proofType,
      wallet_proof: input.walletProof || null,
      wallet_proof_chain_id: input.walletProofChainId || null,
      wallet_proof_deadline: input.walletProofDeadline || null,
      updated_at: now,
    };
    store.state.rows.set(identityKey(input.agentRegistry, input.agentId), row);
    persistFallbackState(store.state);
    store.close();
    return mapIdentity(row);
  }

  const stmt = store.db.prepare(`
    UPDATE agent_identities
    SET agent_wallet = ?,
        wallet_proof_type = ?,
        wallet_proof = ?,
        wallet_proof_chain_id = ?,
        wallet_proof_deadline = ?,
        updated_at = ?
    WHERE agent_registry = ? AND agent_id = ?
  `);

  stmt.run(
    input.agentWallet,
    proofType,
    input.walletProof || null,
    input.walletProofChainId || null,
    input.walletProofDeadline || null,
    now,
    input.agentRegistry,
    input.agentId,
  );

  const record = store.db
    .prepare('SELECT * FROM agent_identities WHERE agent_registry = ? AND agent_id = ?')
    .get(input.agentRegistry, input.agentId);
  store.close();

  if (!record) {
    throw new Error(`Agent identity not found for ${input.agentRegistry}:${input.agentId}`);
  }

  return mapIdentity(record);
}

export function getIdentity(dbPath, agentRegistry, agentId, options = {}) {
  const store = openStore(dbPath, options);
  const record = store.state
    ? selectIdentity(store.state, agentRegistry, agentId)
    : store.db
        .prepare('SELECT * FROM agent_identities WHERE agent_registry = ? AND agent_id = ?')
        .get(agentRegistry, agentId);
  store.close();
  return mapIdentity(record);
}

export function getIdentityByWallet(dbPath, wallet, options = {}) {
  const store = openStore(dbPath, options);
  const record = store.state
    ? selectIdentityByWallet(store.state, wallet)
    : store.db.prepare('SELECT * FROM agent_identities WHERE agent_wallet = ?').get(wallet);
  store.close();
  return mapIdentity(record);
}

export function listIdentities(dbPath, filter = {}, options = {}) {
  const limit = filter.limit || 50;
  const store = openStore(dbPath, options);

  if (store.state) {
    const rows = sortIdentitiesByUpdatedAt(store.state.rows.values())
      .filter(
        (row) =>
          (!filter.agentRegistry || row.agent_registry === filter.agentRegistry) &&
          (!filter.agentId || row.agent_id === filter.agentId) &&
          (!filter.agentWallet || row.agent_wallet === filter.agentWallet) &&
          (filter.active === undefined ||
            filter.active === null ||
            row.active === (filter.active ? 1 : 0)),
      )
      .slice(0, limit);
    store.close();
    return rows.map(mapIdentity);
  }

  const conditions = [];
  const params = [];

  if (filter.agentRegistry) {
    conditions.push('agent_registry = ?');
    params.push(filter.agentRegistry);
  }
  if (filter.agentId) {
    conditions.push('agent_id = ?');
    params.push(filter.agentId);
  }
  if (filter.agentWallet) {
    conditions.push('agent_wallet = ?');
    params.push(filter.agentWallet);
  }
  if (filter.active !== undefined && filter.active !== null) {
    conditions.push('active = ?');
    params.push(filter.active ? 1 : 0);
  }

  const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';

  const rows = store.db
    .prepare(
      `
    SELECT * FROM agent_identities
    ${where}
    ORDER BY updated_at DESC
    LIMIT ?
  `,
    )
    .all(...params, limit);

  store.close();
  return rows.map(mapIdentity);
}

export default {
  registerIdentity,
  setAgentWallet,
  getIdentity,
  getIdentityByWallet,
  listIdentities,
};
