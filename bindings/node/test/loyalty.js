/**
 * Loyalty API tests for @stateset/embedded Node.js bindings.
 *
 * Points are integers; reward `value` is an exact decimal string.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

test('Loyalty: programs, accounts, points, rewards', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('loyalty API exists', () => {
    assert.ok(commerce.loyalty, 'loyalty API should exist');
  });

  let program;
  await t.test('create_program returns a program', async () => {
    program = await commerce.loyalty.createProgram({
      name: 'Rewards Club',
      description: 'Earn points on every order',
      pointsPerDollar: 2,
      // A tier is accepted on the input; note the SQLite backend does not yet
      // persist tiers (loyalty_programs has no tiers column), so the returned
      // `tiers` is empty. Tracked as a separate backend gap.
      tiers: [{ name: 'Silver', minPoints: 0, multiplier: 1.0, perks: ['free shipping'] }],
    });
    assert.equal(program.name, 'Rewards Club');
    assert.equal(program.pointsPerDollar, 2);
    assert.ok(Array.isArray(program.tiers));
    assert.ok(program.id);
  });

  let account;
  await t.test('enroll creates an account for a customer', async () => {
    const customer = await commerce.customers.create({
      email: 'loyal@example.com',
      firstName: 'Loyal',
      lastName: 'Customer',
    });
    account = await commerce.loyalty.enroll({
      customerId: customer.id,
      programId: program.id,
    });
    assert.equal(account.customerId, customer.id);
    assert.equal(account.programId, program.id);
    assert.equal(account.pointsBalance, 0);
  });

  await t.test('adjust_points earns points and updates the balance', async () => {
    const txn = await commerce.loyalty.adjustPoints({
      accountId: account.id,
      points: 150,
      transactionType: 'earn',
      referenceId: 'order-1',
      description: 'Purchase',
    });
    assert.equal(txn.points, 150);
    assert.equal(txn.transactionType, 'earn');

    const updated = await commerce.loyalty.getAccount(account.id);
    assert.equal(updated.pointsBalance, 150);
  });

  await t.test('get_transactions returns the earn transaction', async () => {
    const txns = await commerce.loyalty.getTransactions(account.id);
    assert.ok(txns.length >= 1);
    assert.equal(txns[0].points, 150);
  });

  let reward;
  await t.test('create_reward with an exact-string value', async () => {
    reward = await commerce.loyalty.createReward({
      programId: program.id,
      name: '$5 off',
      pointsCost: 100,
      rewardType: 'discount',
      value: '5.00',
    });
    assert.equal(reward.name, '$5 off');
    assert.equal(reward.pointsCost, 100);
    assert.equal(reward.rewardType, 'discount');
    assert.equal(reward.value, '5.00');
    assert.equal(reward.isActive, true);
  });

  await t.test('get_reward and list_rewards find the reward', async () => {
    const got = await commerce.loyalty.getReward(reward.id);
    assert.equal(got.id, reward.id);
    const rewards = await commerce.loyalty.listRewards();
    assert.ok(rewards.some((r) => r.id === reward.id));
  });

  await t.test('delete_reward removes it', async () => {
    await commerce.loyalty.deleteReward(reward.id);
    const gone = await commerce.loyalty.getReward(reward.id);
    assert.equal(gone, null);
  });
});
