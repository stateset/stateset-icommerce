import { afterEach, describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { createServer } from 'node:http';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { JsonRpcProvider } from 'ethers';
import { executePayment, getBalance, getTransactionStatus } from '../../src/chains/stablecoin.js';
import { getKeyManager } from '../../src/sync/keys.js';

const ADDRESS = '0x1111111111111111111111111111111111111111';
const TX_HASH = `0x${'a'.repeat(64)}`;
const runNode = promisify(execFile);

afterEach(() => mock.restoreAll());

function trackCleanup() {
  const destroyed = [];
  const original = JsonRpcProvider.prototype.destroy;
  mock.method(JsonRpcProvider.prototype, 'destroy', function () {
    destroyed.push(this);
    return original.call(this);
  });
  return (expected = 1) => {
    assert.equal(destroyed.length, expected);
    assert.equal(new Set(destroyed).size, expected);
    assert.ok(destroyed.every((provider) => provider.destroyed));
  };
}

function mockTransactionBuild() {
  mock.method(JsonRpcProvider.prototype, 'getFeeData', async () => ({ gasPrice: 1n }));
  mock.method(JsonRpcProvider.prototype, 'getTransactionCount', async () => 2);
  mock.method(JsonRpcProvider.prototype, 'estimateGas', async () => 21_000n);
}

async function withPayment(options, operation) {
  const configDir = await fs.mkdtemp(path.join(os.tmpdir(), 'stateset-evm-lifecycle-'));
  try {
    await getKeyManager(configDir).ensureKeys('lifecycle-agent');
    const result = await executePayment(
      {
        agentId: 'lifecycle-agent',
        chainId: 'base',
        toAddress: ADDRESS,
        amount: '0.001',
        tokenSymbol: options.tokenSymbol || 'ETH',
      },
      { configDir, simulate: options.simulate ?? true },
    );
    await operation(result);
  } finally {
    await fs.rm(configDir, { recursive: true, force: true });
  }
}

describe('EVM provider lifecycle', () => {
  it('closes the provider after a native balance lookup', async () => {
    const checkCleanup = trackCleanup();
    mock.method(JsonRpcProvider.prototype, 'getBalance', async () => 10n ** 18n);
    const result = await getBalance(ADDRESS, 'base', 'ETH');
    assert.equal(result.balanceSmallest, 10n ** 18n);
    assert.equal(result.symbol, 'ETH');
    checkCleanup();
  });

  it('closes the provider after an ERC20 balance lookup', async () => {
    const checkCleanup = trackCleanup();
    mock.method(
      JsonRpcProvider.prototype,
      'call',
      async () => `0x${1_234_567n.toString(16).padStart(64, '0')}`,
    );
    const result = await getBalance(ADDRESS, 'base', 'USDC');
    assert.equal(result.balanceSmallest, 1_234_567n);
    assert.equal(result.balance, '1.234567');
    checkCleanup();
  });

  it('preserves a native balance error while closing its provider', async () => {
    const checkCleanup = trackCleanup();
    const error = new Error('native RPC unavailable');
    mock.method(JsonRpcProvider.prototype, 'getBalance', async () => {
      throw error;
    });
    await assert.rejects(getBalance(ADDRESS, 'base', 'ETH'), error);
    checkCleanup();
  });

  it('closes the provider when an ERC20 balance call fails', async () => {
    const checkCleanup = trackCleanup();
    mock.method(JsonRpcProvider.prototype, 'call', async () => {
      throw new Error('token RPC unavailable');
    });
    await assert.rejects(getBalance(ADDRESS, 'base', 'USDC'), /token RPC unavailable/);
    checkCleanup();
  });

  it('closes the provider when the transaction receipt is absent', async () => {
    const checkCleanup = trackCleanup();
    mock.method(JsonRpcProvider.prototype, 'getTransactionReceipt', async () => null);
    assert.deepEqual(await getTransactionStatus(TX_HASH, 'base'), {
      confirmed: false,
      confirmations: 0,
    });
    checkCleanup();
  });

  it('keeps the provider alive through the confirmation lookup, then closes it', async () => {
    const checkCleanup = trackCleanup();
    mock.method(JsonRpcProvider.prototype, 'getTransactionReceipt', async () => ({
      status: 1,
      blockNumber: 10,
    }));
    mock.method(JsonRpcProvider.prototype, 'getBlockNumber', async function () {
      assert.equal(this.destroyed, false);
      return 12;
    });
    const result = await getTransactionStatus(TX_HASH, 'base');
    assert.equal(result.confirmations, 3);
    assert.equal(result.blockNumber, 10);
    checkCleanup();
  });

  it('closes the provider when an on-chain receipt reports a revert', async () => {
    const checkCleanup = trackCleanup();
    mock.method(JsonRpcProvider.prototype, 'getTransactionReceipt', async () => ({ status: 0 }));
    await assert.rejects(getTransactionStatus(TX_HASH, 'base'), /reverted on-chain/);
    checkCleanup();
  });

  it('preserves receipt RPC errors while closing the provider', async () => {
    const checkCleanup = trackCleanup();
    const error = new Error('receipt RPC unavailable');
    mock.method(JsonRpcProvider.prototype, 'getTransactionReceipt', async () => {
      throw error;
    });
    await assert.rejects(getTransactionStatus(TX_HASH, 'base'), error);
    checkCleanup();
  });

  for (const tokenSymbol of ['ETH', 'USDC']) {
    it(`closes the provider after building a ${tokenSymbol} preview`, async () => {
      const checkCleanup = trackCleanup();
      mockTransactionBuild();
      await withPayment({ tokenSymbol }, (result) => {
        assert.equal(result.success, true, result.error);
        assert.equal(result.simulated, true);
        assert.equal(
          result.txPreview.type,
          tokenSymbol === 'ETH' ? 'evm_native_transfer' : 'evm_erc20_transfer',
        );
        assert.equal(result.txPreview.request.gasLimit, 25_200n);
      });
      checkCleanup();
    });
  }

  it('closes the provider when transaction construction fails', async () => {
    const checkCleanup = trackCleanup();
    mock.method(JsonRpcProvider.prototype, 'getFeeData', async () => {
      throw new Error('fee RPC unavailable');
    });
    await withPayment({}, (result) => {
      assert.equal(result.success, false);
      assert.equal(result.error, 'fee RPC unavailable');
    });
    checkCleanup();
  });

  it('closes both construction and broadcast providers when submission fails', async () => {
    const checkCleanup = trackCleanup();
    mockTransactionBuild();
    mock.method(JsonRpcProvider.prototype, 'broadcastTransaction', async () => {
      throw new Error('broadcast RPC unavailable');
    });
    await withPayment({ simulate: false }, (result) => {
      assert.equal(result.success, false);
      assert.equal(result.error, 'broadcast RPC unavailable');
    });
    checkCleanup(2);
  });

  it('closes every provider after an offline signed payment and confirmation', async () => {
    const checkCleanup = trackCleanup();
    mockTransactionBuild();
    mock.method(JsonRpcProvider.prototype, 'broadcastTransaction', async (raw) => {
      assert.match(raw, /^0x[0-9a-f]+$/i);
      return { hash: TX_HASH };
    });
    mock.method(JsonRpcProvider.prototype, 'getTransactionReceipt', async () => ({
      status: 1,
      blockNumber: 80,
    }));
    mock.method(JsonRpcProvider.prototype, 'getBlockNumber', async () => 100);
    await withPayment({ simulate: false }, (result) => {
      assert.equal(result.success, true, result.error);
      assert.equal(result.txHash, TX_HASH);
      assert.equal(result.confirmations, 21);
    });
    checkCleanup(3);
  });

  it('lets the process exit after real network discovery fails', { timeout: 20_000 }, async () => {
    let requests = 0;
    const server = createServer((_request, response) => {
      requests++;
      response.writeHead(503, { 'content-type': 'application/json' });
      response.end(JSON.stringify({ error: 'offline RPC fixture' }));
    });
    await new Promise((resolve, reject) => {
      server.once('error', reject);
      server.listen(0, '127.0.0.1', resolve);
    });
    try {
      const { stdout } = await runNode(
        process.execPath,
        [
          '--input-type=module',
          '-e',
          `
        import { getBalance } from './src/chains/stablecoin.js';
        try {
          await getBalance('${ADDRESS}', 'base', 'ETH');
          process.exitCode = 1;
        } catch {
          process.stdout.write('RPC_REFUSED\\n');
        }
      `,
        ],
        {
          cwd: fileURLToPath(new URL('../../', import.meta.url)),
          env: { ...process.env, BASE_RPC_URL: `http://127.0.0.1:${server.address().port}` },
          timeout: 10_000,
        },
      );
      assert.match(stdout, /RPC_REFUSED/);
      assert.ok(requests > 0, 'the child must contact the local refusing RPC fixture');
    } finally {
      await new Promise((resolve) => server.close(resolve));
    }
  });
});
