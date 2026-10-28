'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { Commerce } = require('../index.js');

test('posting settings round-trip and reject invalid replacements atomically', async () => {
  const commerce = new Commerce(':memory:');
  try {
    const gl = commerce.generalLedger;
    assert.equal(await gl.getAutoPostingConfig(), null);
    await gl.initializeChartOfAccounts();
    const accounts = await gl.listAccounts();
    const account = (number) => accounts.find((a) => a.accountNumber === number).id;
    const config = {
      configName: 'Native', cashAccountId: account('1010'), accountsReceivableAccountId: account('1100'),
      inventoryAccountId: account('1200'), accountsPayableAccountId: account('2010'),
      salesRevenueAccountId: account('4010'), cogsAccountId: account('5010'),
      autoPostDepreciation: true, autoPostRevenueRecognition: true,
    };
    const saved = await gl.setAutoPostingConfig(config);
    assert.equal(saved.autoPostDepreciation, true);
    assert.equal((await gl.getAutoPostingConfig()).id, saved.id);
    await assert.rejects(gl.setAutoPostingConfig({ ...config, cashAccountId: config.salesRevenueAccountId }));
    assert.equal((await gl.getAutoPostingConfig()).id, saved.id);
  } finally {
    await commerce.close();
  }
});
