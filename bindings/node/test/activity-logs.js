/**
 * Activity log API tests for @stateset/embedded Node.js bindings.
 *
 * Append-only subject history: record, get, list with filters, and
 * per-subject history.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { randomUUID } = require('node:crypto');

test('ActivityLogs: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');
  const subjectId = randomUUID();

  await t.test('activityLogs API exists and is supported', async () => {
    assert.ok(commerce.activityLogs, 'activityLogs API should exist');
    assert.equal(await commerce.activityLogs.isSupported(), true);
  });

  let entry;
  await t.test('record creates an entry', async () => {
    entry = await commerce.activityLogs.record({
      subjectType: 'sales_order',
      subjectId,
      action: 'status_changed',
      summary: 'Status changed from pending to shipped',
      actorKind: 'agent',
      actor: 'agent-1',
      metadata: JSON.stringify({ from: 'pending', to: 'shipped' }),
    });
    assert.ok(entry.id);
    assert.equal(entry.subjectType, 'sales_order');
    assert.equal(entry.subjectId, subjectId);
    assert.equal(entry.action, 'status_changed');
    assert.equal(entry.actorKind, 'agent');
    assert.equal(entry.actor, 'agent-1');
    assert.deepEqual(JSON.parse(entry.metadata), { from: 'pending', to: 'shipped' });
    assert.ok(entry.createdAt);
  });

  await t.test('record rejects an invalid actor kind', async () => {
    await assert.rejects(
      () =>
        commerce.activityLogs.record({
          subjectType: 'sales_order',
          subjectId,
          action: 'created',
          summary: 'nope',
          actorKind: 'not-a-kind',
        }),
      /Invalid actor kind/,
    );
  });

  await t.test('get returns the entry, and null when missing', async () => {
    const found = await commerce.activityLogs.get(entry.id);
    assert.equal(found.id, entry.id);
    assert.equal(await commerce.activityLogs.get(randomUUID()), null);
  });

  await t.test('list filters by subject and action', async () => {
    await commerce.activityLogs.record({
      subjectType: 'product',
      subjectId: randomUUID(),
      action: 'created',
      summary: 'Product created',
    });

    const bySubject = await commerce.activityLogs.list({ subjectType: 'sales_order', subjectId });
    assert.equal(bySubject.length, 1);
    assert.equal(bySubject[0].id, entry.id);

    const byAction = await commerce.activityLogs.list({ action: 'created' });
    assert.equal(byAction.length, 1);
    assert.equal(byAction[0].subjectType, 'product');

    const all = await commerce.activityLogs.list();
    assert.equal(all.length, 2);
  });

  await t.test('historyForSubject returns the subject history', async () => {
    const history = await commerce.activityLogs.historyForSubject('sales_order', subjectId);
    assert.equal(history.length, 1);
    assert.equal(history[0].id, entry.id);

    const empty = await commerce.activityLogs.historyForSubject('sales_order', randomUUID());
    assert.equal(empty.length, 0);
  });
});
