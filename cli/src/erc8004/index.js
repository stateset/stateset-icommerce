/**
 * ERC-8004 Identity Registry helpers (SQLite-backed)
 */

import Database from 'better-sqlite3';
import { randomUUID } from 'node:crypto';

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

function normalizeProofType(value) {
  if (!value) return null;
  const normalized = String(value).toLowerCase();
  if (!PROOF_TYPES.has(normalized)) {
    throw new Error(`Invalid proof type: ${value}. Expected eip712 or erc1271.`);
  }
  return normalized;
}

function openDb(dbPath) {
  const db = new Database(dbPath);
  db.pragma('journal_mode = WAL');
  db.exec(IDENTITY_SCHEMA);
  return db;
}

function mapIdentity(row) {
  if (!row) return null;
  return {
    ...row,
    active: Boolean(row.active),
  };
}

export function registerIdentity(dbPath, input) {
  const db = openDb(dbPath);
  const now = new Date().toISOString();
  const id = input.id || randomUUID();
  const active = input.active === undefined ? 1 : input.active ? 1 : 0;
  const proofType = normalizeProofType(input.walletProofType);

  const stmt = db.prepare(`
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
    now,
    now,
  );

  const record = db
    .prepare('SELECT * FROM agent_identities WHERE agent_registry = ? AND agent_id = ?')
    .get(input.agentRegistry, input.agentId);
  db.close();
  return mapIdentity(record);
}

export function setAgentWallet(dbPath, input) {
  const db = openDb(dbPath);
  const now = new Date().toISOString();
  const proofType = normalizeProofType(input.walletProofType);

  const stmt = db.prepare(`
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

  const record = db
    .prepare('SELECT * FROM agent_identities WHERE agent_registry = ? AND agent_id = ?')
    .get(input.agentRegistry, input.agentId);
  db.close();

  if (!record) {
    throw new Error(`Agent identity not found for ${input.agentRegistry}:${input.agentId}`);
  }

  return mapIdentity(record);
}

export function getIdentity(dbPath, agentRegistry, agentId) {
  const db = openDb(dbPath);
  const record = db
    .prepare('SELECT * FROM agent_identities WHERE agent_registry = ? AND agent_id = ?')
    .get(agentRegistry, agentId);
  db.close();
  return mapIdentity(record);
}

export function getIdentityByWallet(dbPath, wallet) {
  const db = openDb(dbPath);
  const record = db.prepare('SELECT * FROM agent_identities WHERE agent_wallet = ?').get(wallet);
  db.close();
  return mapIdentity(record);
}

export function listIdentities(dbPath, filter = {}) {
  const db = openDb(dbPath);
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
  const limit = filter.limit || 50;

  const rows = db
    .prepare(
      `
    SELECT * FROM agent_identities
    ${where}
    ORDER BY updated_at DESC
    LIMIT ?
  `,
    )
    .all(...params, limit);

  db.close();
  return rows.map(mapIdentity);
}

export default {
  registerIdentity,
  setAgentWallet,
  getIdentity,
  getIdentityByWallet,
  listIdentities,
};
