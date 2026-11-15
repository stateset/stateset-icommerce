/**
 * x402_credit_balance and resolve_payment_link read like lookups but write
 * rows: the first balance query for a payer opens a zero-balance credit
 * account (the native getCreditBalance is a get-or-create), and every resolve
 * counts a payment link view. Both are `write` tools, so without --apply they
 * return a preview and change nothing. x402_get_credit_account and
 * get_payment_link_status are the pure reads of the same records, and run in
 * preview mode.
 */

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { Commerce } from '../../../bindings/node/index.js';
import { createStatesetMcpServer } from '../../src/mcp-server.js';

function fixture(t) {
  const directory = mkdtempSync(join(tmpdir(), 'stateset-lookup-writes-'));
  const dbPath = join(directory, 'store.db');
  const commerce = new Commerce(dbPath);
  const options = { commerce, dbPath, toolProfile: 'all' };
  const preview = createStatesetMcpServer(options);
  const apply = createStatesetMcpServer({ ...options, allowApply: true });
  t.after(async () => {
    for (const server of [preview, apply]) {
      server.dispose?.();
      await server.close?.().catch(() => {});
    }
    commerce.close();
    rmSync(directory, { recursive: true, force: true });
  });
  return { preview, apply };
}

function assertPreview(outcome, tool, params) {
  assert.equal(outcome.status, 'preview', JSON.stringify(outcome));
  assert.equal(outcome.preview, true);
  assert.equal(outcome.notice?.code, 'APPLY_REQUIRED');
  assert.deepEqual(outcome.wouldDo, { tool, params });
  assert.equal(outcome.result, null, 'the handler did not run');
}

test('x402_credit_balance previews without --apply and opens the account only with it', async (t) => {
  const { preview, apply } = fixture(t);
  const params = { payerAddress: '0xLookupPayer', asset: 'usdc', network: 'base' };
  const account = () => preview.executeTool('x402_get_credit_account', params);

  assertPreview(
    await preview.executeTool('x402_credit_balance', params),
    'x402_credit_balance',
    params,
  );
  // The pure read runs without --apply; the preview opened no account.
  const untouched = await account();
  assert.equal(untouched.permission?.allowed, true, JSON.stringify(untouched));
  assert.equal(untouched.result?.error, 'Credit account not found');

  const applied = await apply.executeTool('x402_credit_balance', params);
  assert.equal(applied.status, 'success', JSON.stringify(applied));
  assert.equal(applied.result.success, true);
  assert.equal(Number(applied.result.balance), 0);

  const opened = await account();
  assert.equal(opened.status, 'success', 'the balance query opened the account');
  assert.equal(opened.result.success, true);
  assert.equal(Number(opened.result.account.balance), 0);
});

test('resolve_payment_link previews without --apply and counts a view only with it', async (t) => {
  const { preview, apply } = fixture(t);
  const created = await apply.executeTool('create_payment_link', {
    items: [{ name: 'Lookup mug', quantity: 1, unitPrice: 12 }],
  });
  assert.equal(created.result.success, true, JSON.stringify(created));
  const { linkId, shortCode } = created.result;
  const views = async () => {
    const status = await preview.executeTool('get_payment_link_status', { linkId });
    assert.equal(status.status, 'success', 'the pure read runs without --apply');
    assert.equal(status.result.success, true, JSON.stringify(status));
    return status.result.views;
  };

  assert.equal(await views(), 0);
  assertPreview(
    await preview.executeTool('resolve_payment_link', { linkId: shortCode }),
    'resolve_payment_link',
    { linkId: shortCode },
  );
  assert.equal(await views(), 0, 'a preview must not count a view');

  const resolved = await apply.executeTool('resolve_payment_link', { linkId: shortCode });
  assert.equal(resolved.status, 'success', JSON.stringify(resolved));
  assert.equal(resolved.result.success, true);
  assert.equal(resolved.result.link.id, linkId);
  assert.equal(resolved.result.total, 12);
  assert.equal(await views(), 1);
});
