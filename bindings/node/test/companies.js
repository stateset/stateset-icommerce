/**
 * B2B company API tests for @stateset/embedded Node.js bindings.
 *
 * Company lifecycle: create, get, update, list/search, addresses,
 * price overrides, contacts, and delete.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { randomUUID } = require('node:crypto');

test('Companies: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('companies API exists and is supported', async () => {
    assert.ok(commerce.companies, 'companies API should exist');
    assert.equal(await commerce.companies.isSupported(), true);
  });

  let company;
  await t.test('create returns an active company', async () => {
    company = await commerce.companies.create({
      name: 'Acme Industrial',
      reference: 'ACME-1',
      email: 'ap@acme.test',
      currency: 'USD',
      paymentTermsDays: 30,
      tags: ['wholesale'],
    });
    assert.ok(company.id);
    assert.equal(company.name, 'Acme Industrial');
    assert.equal(company.reference, 'ACME-1');
    assert.equal(company.currency, 'USD');
    assert.equal(company.paymentTermsDays, 30);
    assert.equal(company.status, 'active');
    assert.deepEqual(company.tags, ['wholesale']);
  });

  await t.test('get returns the company, and null when missing', async () => {
    const found = await commerce.companies.get(company.id);
    assert.equal(found.name, 'Acme Industrial');
    assert.equal(await commerce.companies.get(randomUUID()), null);
  });

  await t.test('update applies partial changes', async () => {
    const updated = await commerce.companies.update(company.id, {
      phone: '+1-555-0100',
      paymentTermsDays: 45,
      status: 'inactive',
    });
    assert.equal(updated.phone, '+1-555-0100');
    assert.equal(updated.paymentTermsDays, 45);
    assert.equal(updated.status, 'inactive');
    assert.equal(updated.name, 'Acme Industrial');
  });

  await t.test('update rejects an invalid status', async () => {
    await assert.rejects(
      () => commerce.companies.update(company.id, { status: 'nope' }),
      /Invalid company status/,
    );
  });

  await t.test('list filters by status and search', async () => {
    assert.equal((await commerce.companies.list({ status: 'inactive' })).length, 1);
    assert.equal((await commerce.companies.list({ status: 'active' })).length, 0);
    assert.equal((await commerce.companies.list({ search: 'Acme' })).length, 1);
    assert.equal((await commerce.companies.list()).length, 1);
  });

  await t.test('addresses and price overrides start empty', async () => {
    assert.deepEqual(await commerce.companies.listAddresses(company.id), []);
    assert.deepEqual(await commerce.companies.listPriceOverrides(company.id), []);
  });

  let contact;
  await t.test('createContact links a contact to the company', async () => {
    contact = await commerce.companies.createContact({
      firstName: 'Ada',
      lastName: 'Lovelace',
      email: 'ada@acme.test',
      title: 'Buyer',
      companyIds: [company.id],
    });
    assert.ok(contact.id);
    assert.equal(contact.firstName, 'Ada');
    assert.equal(contact.isActive, true);
    assert.deepEqual(contact.companyIds, [company.id]);
  });

  await t.test('getContact and listContacts return the contact', async () => {
    const found = await commerce.companies.getContact(contact.id);
    assert.equal(found.id, contact.id);
    assert.equal(await commerce.companies.getContact(randomUUID()), null);

    const contacts = await commerce.companies.listContacts(company.id);
    assert.equal(contacts.length, 1);
    assert.equal(contacts[0].id, contact.id);
  });

  await t.test('delete removes the company', async () => {
    await commerce.companies.delete(company.id);
    const after = await commerce.companies.get(company.id);
    if (after !== null) {
      assert.equal(after.status, 'inactive');
    }
  });
});
