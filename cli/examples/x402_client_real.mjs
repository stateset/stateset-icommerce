#!/usr/bin/env node
/**
 * Real x402 Client Example (Sequencer-backed)
 */

import { loadSyncConfig, SyncConfig, getKeysDir } from '../src/sync/config.js';
import { AgentKeyManager } from '../src/sync/keys.js';
import { X402SequencerClient } from '../src/x402/client.js';
import { createX402Agent } from '../src/x402/agent.js';

const SERVER_URL = process.env.X402_SERVER_URL || 'http://localhost:8402/premium';
const PAYER_ADDRESS = process.env.X402_PAYER_ADDRESS;
const REQUIRE_RECEIPT = process.env.X402_REQUIRE_RECEIPT === 'true';

if (!PAYER_ADDRESS) {
  console.error('X402_PAYER_ADDRESS is required');
  process.exit(1);
}

const configData = loadSyncConfig(process.cwd());
if (!configData) {
  console.error('Missing .stateset/sync.json. Run stateset sync init or set SEQUENCER_URL.');
  process.exit(1);
}

const syncConfig = new SyncConfig(configData);
const keyManager = new AgentKeyManager(getKeysDir(process.cwd(), syncConfig.keysDir));
const { signingKey } = await keyManager.ensureKeys(syncConfig.agentId);

const sequencer = new X402SequencerClient(syncConfig);
const agent = createX402Agent({
  sequencerClient: sequencer,
  tenantId: syncConfig.tenantId,
  storeId: syncConfig.storeId,
  agentId: syncConfig.agentId,
  agentKeyId: signingKey.keyId,
  payerAddress: PAYER_ADDRESS,
  signingKey,
  requireReceipt: REQUIRE_RECEIPT,
});

const response = await agent.fetch(SERVER_URL, { method: 'GET' });
const body = await response.text();

console.log(`Status: ${response.status}`);
console.log(body);
