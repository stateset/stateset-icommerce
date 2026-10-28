import { test } from 'node:test';
import assert from 'node:assert/strict';
import { generalLedgerTools } from '../../src/tools/general-ledger.js';

test('posting configuration writes require apply and preserve account settings', async () => {
  const tool = generalLedgerTools.find((t) => t.name === 'set_gl_posting_config');
  const params = {
    configName: 'Ledger',
    salesTaxPayableAccountId: 'tax',
    autoPostDepreciation: true,
  };
  let writes = 0;
  const commerce = {
    generalLedger: {
      setAutoPostingConfig: async (input) => {
        writes += 1;
        assert.deepEqual(input, params);
        return { id: 'configuration', ...input };
      },
    },
  };
  await tool.handler({ commerce, params, allowApply: false });
  assert.equal(writes, 0);
  const applied = await tool.handler({ commerce, params, allowApply: true });
  assert.equal(writes, 1);
  assert.equal(applied.config.salesTaxPayableAccountId, 'tax');
});

test('posting configuration reads report an unconfigured store explicitly', async () => {
  const tool = generalLedgerTools.find((t) => t.name === 'get_gl_posting_config');
  const result = await tool.handler({
    commerce: { generalLedger: { getAutoPostingConfig: async () => null } },
  });
  assert.deepEqual(result, { success: true, config: null });
});
