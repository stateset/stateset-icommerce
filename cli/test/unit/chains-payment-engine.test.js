import { afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { getKeyManager } from '../../src/sync/keys.js';
import { executePayment, getBalance } from '../../src/chains/stablecoin.js';
import { deriveWallet, getWalletAddress } from '../../src/chains/wallet.js';

const ORIGINAL_FETCH = global.fetch;
const ORIGINAL_ZCASH_WALLET_RPC_URL = process.env.ZCASH_WALLET_RPC_URL;

function jsonResponse(payload, status = 200) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

function textResponse(body, status = 200) {
  return new Response(body, {
    status,
    headers: { 'content-type': 'text/plain' },
  });
}

afterEach(() => {
  global.fetch = ORIGINAL_FETCH;
  if (ORIGINAL_ZCASH_WALLET_RPC_URL === undefined) {
    delete process.env.ZCASH_WALLET_RPC_URL;
  } else {
    process.env.ZCASH_WALLET_RPC_URL = ORIGINAL_ZCASH_WALLET_RPC_URL;
  }
});

describe('chain payment engine', () => {
  it('executes a native Bitcoin payment via the shared payment engine', async () => {
    const configDir = await fs.mkdtemp(path.join(os.tmpdir(), 'stateset-btc-'));
    const agentId = 'btc-agent';
    const txHash = 'a'.repeat(64);
    const fundingTxId = 'b'.repeat(64);

    try {
      await getKeyManager(configDir).ensureKeys(agentId);
      const wallet = await deriveWallet(agentId, 'bitcoin', { configDir });
      const fromAddress = await getWalletAddress(agentId, 'bitcoin', { configDir });
      assert.match(fromAddress, /^bc1q/i);

      global.fetch = async (input, init = {}) => {
        const url = typeof input === 'string' ? input : input.url;

        if (url.endsWith('/fee-estimates')) {
          return jsonResponse({ 1: 4 });
        }

        if (url.endsWith(`/address/${fromAddress}/utxo`)) {
          return jsonResponse([
            {
              txid: fundingTxId,
              vout: 0,
              value: 150_000_000,
              status: { confirmed: true, block_height: 90, block_time: 1_710_000_000 },
            },
          ]);
        }

        if (wallet.legacyAddress && url.endsWith(`/address/${wallet.legacyAddress}/utxo`)) {
          return jsonResponse([]);
        }

        if (url.endsWith('/tx') && init.method === 'POST') {
          assert.equal(typeof init.body, 'string');
          assert.ok(init.body.length > 0);
          return textResponse(txHash);
        }

        if (url.endsWith(`/tx/${txHash}/status`)) {
          return jsonResponse({
            confirmed: true,
            block_height: 100,
            block_time: 1_710_000_100,
          });
        }

        if (url.endsWith('/blocks/tip/height')) {
          return textResponse('105');
        }

        throw new Error(`Unexpected Bitcoin fetch: ${url}`);
      };

      const balance = await getBalance(fromAddress, 'bitcoin');
      assert.equal(balance.symbol, 'BTC');
      assert.equal(balance.balance, '1.50000000');

      const result = await executePayment(
        {
          agentId,
          chainId: 'bitcoin',
          toAddress: '1BoatSLRHtKNngkdXEeobR76b53LETtpyT',
          amount: 0.5,
        },
        {
          configDir,
          simulate: false,
        },
      );

      assert.equal(result.success, true);
      assert.equal(result.txHash, txHash);
      assert.equal(result.confirmations, 6);
      assert.equal(result.blockNumber, 100);
    } finally {
      await fs.rm(configDir, { recursive: true, force: true });
    }
  });

  it('executes a shielded Zcash payment via wallet RPC-backed unified addresses', async () => {
    const configDir = await fs.mkdtemp(path.join(os.tmpdir(), 'stateset-zec-'));
    const agentId = 'zec-agent';
    const shieldedAddress = `u1${'a'.repeat(60)}`;
    const recipientAddress = `u1${'b'.repeat(60)}`;
    const txHash = 'c'.repeat(64);
    process.env.ZCASH_WALLET_RPC_URL = 'http://zcash.example/rpc';

    try {
      global.fetch = async (input, init = {}) => {
        const url = typeof input === 'string' ? input : input.url;
        assert.equal(url, 'http://zcash.example/rpc');

        const request = JSON.parse(init.body);
        switch (request.method) {
          case 'z_getnewaccount':
            return jsonResponse({ result: 7, error: null, id: request.id });
          case 'z_getaddressforaccount':
            return jsonResponse({ result: { address: shieldedAddress }, error: null, id: request.id });
          case 'z_sendmany':
            assert.equal(request.params[0], shieldedAddress);
            assert.equal(request.params[1][0].address, recipientAddress);
            return jsonResponse({ result: 'opid-123', error: null, id: request.id });
          case 'z_getoperationstatus':
            return jsonResponse({
              result: [{ id: 'opid-123', status: 'success', result: { txid: txHash } }],
              error: null,
              id: request.id,
            });
          case 'gettransaction':
            return jsonResponse({
              result: { confirmations: 1, blockheight: 222 },
              error: null,
              id: request.id,
            });
          default:
            throw new Error(`Unexpected Zcash RPC method: ${request.method}`);
        }
      };

      const address = await getWalletAddress(agentId, 'zcash', { configDir, requireShielded: true });
      assert.equal(address, shieldedAddress);

      const result = await executePayment(
        {
          agentId,
          chainId: 'zcash',
          toAddress: recipientAddress,
          amount: 1.25,
          tokenSymbol: 'ZEC',
          metadata: { memo: 'shielded settlement' },
        },
        {
          configDir,
          simulate: false,
        },
      );

      assert.equal(result.success, true);
      assert.equal(result.txHash, txHash);
      assert.equal(result.confirmations, 1);

      const registry = JSON.parse(
        await fs.readFile(path.join(configDir, 'chains', 'zcash-wallets.json'), 'utf8'),
      );
      assert.equal(registry.chains.zcash[agentId].address, shieldedAddress);
    } finally {
      await fs.rm(configDir, { recursive: true, force: true });
    }
  });
});
