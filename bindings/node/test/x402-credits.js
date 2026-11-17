const assert = require('assert');
const { test } = require('node:test');

const { Commerce } = require('../index.js');

// The tool descriptors mark x402.getCreditBalance as a write (see
// READ_ONLY_OVERRIDES in scripts/generate-tool-descriptors.mjs). This pins the
// reason: the balance query is a get-or-create, so it inserts the account,
// while getCreditAccount is a plain read.
test('X402: getCreditBalance creates the credit account; getCreditAccount does not', async () => {
  const commerce = new Commerce(':memory:');
  const input = { payerAddress: '0x00000000000000000000000000000000000000c1' };

  assert.equal(await commerce.x402.getCreditAccount(input), null);
  assert.equal(await commerce.x402.getCreditAccount(input), null, 'a read leaves no account');

  assert.equal(await commerce.x402.getCreditBalance(input), 0);
  const account = await commerce.x402.getCreditAccount(input);
  assert.ok(account, 'the balance query inserted the account');
  assert.equal(account.payerAddress, input.payerAddress);
});
