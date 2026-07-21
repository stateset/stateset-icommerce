/**
 * ERC-8004 API tests for @stateset/embedded Node.js bindings.
 *
 * Identity registration/update/wallet binding, reputation feedback, and the
 * validation request/response flow.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

const REGISTRY = 'eip155:1:0xregistry';

test('Erc8004: identity, reputation and validation lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');
  const agentId = '42';

  await t.test('API exists', () => {
    assert.ok(commerce.erc8004, 'erc8004 API should exist');
  });

  let identity;
  await t.test('registerIdentity stores the agent', async () => {
    identity = await commerce.erc8004.registerIdentity({
      agentRegistry: REGISTRY,
      agentId,
      agentUri: 'https://agents.example/42.json',
      ownerAddress: '0xowner',
    });
    assert.ok(identity.id);
    assert.equal(identity.agentRegistry, REGISTRY);
    assert.equal(identity.agentId, agentId);
    assert.equal(identity.active, true);
  });

  await t.test('getIdentity returns it, and null when missing', async () => {
    const found = await commerce.erc8004.getIdentity(REGISTRY, agentId);
    assert.equal(found.agentUri, 'https://agents.example/42.json');
    assert.equal(await commerce.erc8004.getIdentity(REGISTRY, 'nope'), null);
  });

  await t.test('updateIdentity changes the agent URI', async () => {
    const updated = await commerce.erc8004.updateIdentity(REGISTRY, agentId, {
      agentUri: 'https://agents.example/42-v2.json',
    });
    assert.equal(updated.agentUri, 'https://agents.example/42-v2.json');
  });

  await t.test('setAgentWallet binds a wallet with proof, chain id as a string', async () => {
    const bound = await commerce.erc8004.setAgentWallet(REGISTRY, agentId, '0xwallet', {
      proofType: 'erc1271',
      proof: '0xsig',
      proofChainId: '8453',
      proofDeadline: '2027-01-01T00:00:00Z',
    });
    assert.equal(bound.agentWallet, '0xwallet');
    assert.equal(bound.walletProofChainId, '8453');
    const byWallet = await commerce.erc8004.getIdentityByWallet('0xwallet');
    assert.equal(byWallet.agentId, agentId);
  });

  await t.test('setAgentWallet rejects an unknown proof type', async () => {
    await assert.rejects(
      () =>
        commerce.erc8004.setAgentWallet(REGISTRY, agentId, '0xwallet', {
          proofType: 'not_a_proof',
        }),
      /Invalid agent wallet proof type/,
    );
  });

  await t.test('clearAgentWallet removes the binding', async () => {
    const cleared = await commerce.erc8004.clearAgentWallet(REGISTRY, agentId);
    assert.equal(cleared.agentWallet ?? null, null);
  });

  await t.test('listIdentities and countIdentities agree', async () => {
    const identities = await commerce.erc8004.listIdentities({ agentRegistry: REGISTRY });
    assert.ok(identities.some((i) => i.agentId === agentId));
    assert.equal(await commerce.erc8004.countIdentities({ agentRegistry: REGISTRY }), '1');
  });

  let feedback;
  await t.test('giveFeedback records a signed value as a string', async () => {
    feedback = await commerce.erc8004.giveFeedback({
      agentRegistry: REGISTRY,
      agentId,
      clientAddress: '0xclient',
      value: '95',
      valueDecimals: 2,
      tag1: 'quality',
    });
    assert.equal(feedback.value, '95');
    assert.equal(feedback.valueDecimals, 2);
    assert.equal(feedback.isRevoked, false);
    assert.equal(typeof feedback.feedbackIndex, 'string');
  });

  await t.test('readFeedback and readAllFeedback return the entry', async () => {
    const read = await commerce.erc8004.readFeedback(
      REGISTRY,
      agentId,
      '0xclient',
      feedback.feedbackIndex,
    );
    assert.equal(read.value, '95');
    const all = await commerce.erc8004.readAllFeedback({ agentRegistry: REGISTRY, agentId });
    assert.equal(all.length, 1);
  });

  await t.test('feedbackSummary aggregates as decimal strings', async () => {
    const summary = await commerce.erc8004.feedbackSummary(
      REGISTRY,
      agentId,
      ['0xclient'],
      null,
      null,
    );
    assert.equal(summary.count, '1');
    assert.equal(typeof summary.summaryValue, 'string');
  });

  await t.test('revokeFeedback marks the entry revoked', async () => {
    const revoked = await commerce.erc8004.revokeFeedback(
      REGISTRY,
      agentId,
      '0xclient',
      feedback.feedbackIndex,
    );
    assert.equal(revoked.isRevoked, true);
  });

  const requestHash = '0xrequesthash';
  await t.test('requestValidation stores the request', async () => {
    const request = await commerce.erc8004.requestValidation({
      requestHash,
      agentRegistry: REGISTRY,
      agentId,
      validatorAddress: '0xvalidator',
      requestUri: 'https://validators.example/req',
    });
    assert.equal(request.requestHash, requestHash);
    assert.equal(request.validatorAddress, '0xvalidator');
  });

  await t.test('respondValidation records a validation score', async () => {
    const response = await commerce.erc8004.respondValidation(requestHash, {
      response: 88,
      responseUri: 'https://validators.example/res',
      tag: 'audit',
    });
    assert.equal(response.response, 88);
    assert.equal(response.tag, 'audit');
  });

  await t.test('respondValidation rejects an out-of-range score', async () => {
    await assert.rejects(
      () => commerce.erc8004.respondValidation(requestHash, { response: 999 }),
      /Invalid response: out of range/,
    );
  });

  await t.test('validationStatus and validationSummary reflect the response', async () => {
    const status = await commerce.erc8004.validationStatus(requestHash);
    assert.equal(status.response, 88);
    assert.equal(status.agentId, agentId);
    assert.equal(await commerce.erc8004.validationStatus('0xmissing'), null);

    const summary = await commerce.erc8004.validationSummary(REGISTRY, agentId, null, null);
    assert.equal(summary.count, '1');
    assert.equal(summary.averageResponse, 88);
  });
});
