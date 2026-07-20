/**
 * Customer Segments API tests for @stateset/embedded Node.js bindings.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

test('Segments: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('segments API exists and is supported', async () => {
    assert.ok(commerce.segments, 'segments API should exist');
    assert.equal(await commerce.segments.isSupported(), true);
  });

  let segment;
  await t.test('create returns a segment with parsed rules', async () => {
    segment = await commerce.segments.create({
      name: 'VIP',
      description: 'High spenders',
      segmentType: 'dynamic',
      rules: [{ field: 'total_spent', operator: 'gte', value: '1000' }],
    });
    assert.equal(segment.name, 'VIP');
    assert.equal(segment.segmentType, 'dynamic');
    assert.equal(segment.rules.length, 1);
    assert.equal(segment.rules[0].field, 'total_spent');
    assert.equal(segment.rules[0].operator, 'gte');
    assert.equal(segment.rules[0].value, '1000');
    assert.equal(segment.memberCount, 0);
    assert.ok(segment.id);
  });

  await t.test('create rejects an invalid operator', async () => {
    await assert.rejects(
      commerce.segments.create({
        name: 'Bad',
        rules: [{ field: 'x', operator: 'nonsense', value: '1' }],
      }),
      /Invalid segment operator/i,
    );
  });

  await t.test('get and update round-trip the rules', async () => {
    const fetched = await commerce.segments.get(segment.id);
    assert.equal(fetched.id, segment.id);
    assert.equal(fetched.rules[0].operator, 'gte');

    const updated = await commerce.segments.update(segment.id, {
      name: 'VIP Renamed',
      rules: [{ field: 'orders', operator: 'gt', value: '5' }],
    });
    assert.equal(updated.name, 'VIP Renamed');
    assert.equal(updated.rules[0].field, 'orders');
    assert.equal(updated.rules[0].operator, 'gt');
  });

  await t.test('member management (add, is_member, list, remove)', async () => {
    const staticSeg = await commerce.segments.create({ name: 'Manual list' });
    const customer = await commerce.customers.create({
      email: 'member@example.com',
      firstName: 'Mem',
      lastName: 'Ber',
    });

    const membership = await commerce.segments.addMember(staticSeg.id, customer.id);
    assert.equal(membership.segmentId, staticSeg.id);
    assert.equal(membership.customerId, customer.id);

    assert.equal(await commerce.segments.isMember(staticSeg.id, customer.id), true);

    const members = await commerce.segments.listMembers(staticSeg.id);
    assert.ok(members.some((m) => m.customerId === customer.id));

    await commerce.segments.removeMember(staticSeg.id, customer.id);
    assert.equal(await commerce.segments.isMember(staticSeg.id, customer.id), false);
  });

  await t.test('list finds segments', async () => {
    const list = await commerce.segments.list();
    assert.ok(list.some((s) => s.id === segment.id));
  });

  await t.test('delete removes the segment', async () => {
    await commerce.segments.delete(segment.id);
    assert.equal(await commerce.segments.get(segment.id), null);
  });
});
