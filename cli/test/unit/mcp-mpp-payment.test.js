// Unit tests for cli/src/mcp/mpp-payment.js
//
// Covers:
//  - `attachPaymentMetadataToResponse`: stamps `_meta.payment` onto a JSON
//    text response; leaves non-JSON / empty / non-text responses untouched.
//  - `createResolveMppPaymentContext`: unpriced tools short-circuit; priced
//    tools without a credential get a payment-required payload; a mismatched
//    credential is rejected with a verification reason; a valid credential
//    authorizes.
//  - `createPreparePaymentForTool`: validation + challenge/credential
//    template construction for the `prepare_payment` runtime tool.

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';

import {
  attachPaymentMetadataToResponse,
  createPreparePaymentForTool,
  createResolveMppPaymentContext,
} from '../../src/mcp/mpp-payment.js';
import { MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE, MPP_PROTOCOL } from '../../src/mpp/index.js';

const SERVICE_INFO = { id: 'svc-test', name: 'Test Service' };
const PRICING = { enabled: true, chainId: 'base', tokenSymbol: 'usdc', amount: '0.25' };

describe('attachPaymentMetadataToResponse', () => {
  it('parses the first text block, attaches payment metadata, and re-serializes', () => {
    const response = {
      content: [
        { type: 'text', text: JSON.stringify({ ok: true }) },
        { type: 'text', text: 'second' },
      ],
    };
    const out = attachPaymentMetadataToResponse(response, { receipt: 'r1' });
    assert.notEqual(out, response);
    assert.deepEqual(JSON.parse(out.content[0].text), {
      ok: true,
      _meta: { payment: { receipt: 'r1' } },
    });
    assert.equal(out.content[1].text, 'second');
  });

  it('returns the response untouched when it has no content, no text block, or invalid JSON', () => {
    assert.equal(attachPaymentMetadataToResponse(null, {}), null);
    const empty = { content: [] };
    assert.equal(attachPaymentMetadataToResponse(empty, {}), empty);
    const image = { content: [{ type: 'image', data: 'x' }] };
    assert.equal(attachPaymentMetadataToResponse(image, {}), image);
    const bad = { content: [{ type: 'text', text: '{not json' }] };
    assert.equal(attachPaymentMetadataToResponse(bad, {}), bad);
  });
});

describe('createResolveMppPaymentContext', () => {
  it('returns an unauthorized, unpriced context when the tool has no pricing', async () => {
    const resolve = createResolveMppPaymentContext({
      getAgenticToolPricing: async () => null,
      serviceInfo: SERVICE_INFO,
    });
    assert.deepEqual(await resolve({ toolName: 'create_order' }), {
      pricing: null,
      challenge: null,
      credential: null,
      authorized: false,
    });
  });

  it('builds a challenge and payment-required payload when no credential is presented', async () => {
    const resolve = createResolveMppPaymentContext({
      getAgenticToolPricing: async () => PRICING,
      serviceInfo: SERVICE_INFO,
    });
    const ctx = await resolve({
      toolName: 'create_order',
      params: { customerId: 'c1' },
      requestId: 'req-1',
      sessionId: 'sess-1',
    });
    assert.equal(ctx.pricing, PRICING);
    assert.equal(ctx.authorized, false);
    assert.equal(ctx.credential, null);
    assert.ok(ctx.challenge?.challengeId);
    assert.equal(ctx.errorPayload.paymentRequired, true);
    assert.equal(ctx.errorPayload.error, MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE);
  });

  it('rejects a credential bound to a different challenge', async () => {
    const resolve = createResolveMppPaymentContext({
      getAgenticToolPricing: async () => PRICING,
      serviceInfo: SERVICE_INFO,
    });
    const ctx = await resolve({
      toolName: 'create_order',
      params: {},
      extra: { _meta: { payment: { protocol: MPP_PROTOCOL, challengeId: 'nope' } } },
    });
    assert.equal(ctx.authorized, false);
    assert.equal(ctx.verification.valid, false);
    assert.equal(ctx.verification.reason, 'Credential challenge does not match');
    assert.equal(
      ctx.errorPayload._meta.payment.validationError,
      'Credential challenge does not match',
    );
  });

  it('authorizes when the presented credential verifies against the challenge', async () => {
    const resolve = createResolveMppPaymentContext({
      getAgenticToolPricing: async () => PRICING,
      serviceInfo: SERVICE_INFO,
    });
    // Challenge ids are deterministic for identical inputs, so derive one
    // from a first pass and replay it as the credential.
    const first = await resolve({ toolName: 'create_order', params: { x: 1 }, requestId: 'r' });
    const credential = {
      protocol: MPP_PROTOCOL,
      challengeId: first.challenge.challengeId,
      payer: 'agent-1',
      amount: first.challenge.amount,
      binding: first.challenge.binding,
      method: first.challenge.paymentMethods?.[0]
        ? {
            kind: first.challenge.paymentMethods[0].kind,
            asset: first.challenge.paymentMethods[0].asset,
            network: first.challenge.paymentMethods[0].network,
          }
        : null,
      authorization: { type: 'signature', signature: '0xabc' },
    };
    const ctx = await resolve({
      toolName: 'create_order',
      params: { x: 1 },
      requestId: 'r',
      extra: { _meta: { payment: credential } },
    });
    assert.equal(ctx.challenge.challengeId, first.challenge.challengeId);
    assert.equal(ctx.authorized, true, ctx.verification.reason);
    assert.equal(ctx.verification.valid, true);
    assert.equal(ctx.credential.protocol, MPP_PROTOCOL);
    assert.equal(ctx.credential.type, 'credential');
    assert.equal('errorPayload' in ctx, false);
  });
});

describe('createPreparePaymentForTool', () => {
  const toolDefsByName = new Map([
    [
      'create_order',
      {
        name: 'create_order',
        description: 'Create an order',
        inputSchema: { customerId: z.string().min(1) },
      },
    ],
  ]);

  const build = (pricing) =>
    createPreparePaymentForTool({
      toolDefsByName,
      getAgenticToolPricing: async () => pricing,
      serviceInfo: SERVICE_INFO,
    });

  it('requires a tool name', async () => {
    assert.deepEqual(await build(PRICING)({ tool: '' }), {
      success: false,
      payable: false,
      error: 'tool is required',
    });
  });

  it('rejects unknown tools', async () => {
    const out = await build(PRICING)({ tool: 'nope' });
    assert.equal(out.success, false);
    assert.equal(out.error, "Unknown tool 'nope'");
  });

  it('normalizes legacy prefixes and reports validation issues', async () => {
    const out = await build(PRICING)({ tool: 'mcp__stateset-commerce__create_order', params: {} });
    assert.equal(out.success, false);
    assert.equal(out.tool, 'create_order');
    assert.equal(out.validation.valid, false);
    assert.equal(out.validation.issues[0].path, 'customerId');
  });

  it('reports non-payable tools and optionally includes the schema', async () => {
    const out = await build(null)({
      tool: 'create_order',
      params: { customerId: 'c1' },
      includeSchema: true,
    });
    assert.equal(out.success, true);
    assert.equal(out.payable, false);
    assert.equal(out.paymentInfo, null);
    assert.equal(out.reason, 'No pricing configured for this tool.');
    assert.equal(out.inputSchema.type, 'object');
  });

  it('builds a challenge, credential template, and retry example for priced tools', async () => {
    const out = await build(PRICING)({
      tool: 'create_order',
      params: { customerId: 'c1' },
      requestId: 'req-9',
    });
    assert.equal(out.success, true);
    assert.equal(out.payable, true);
    assert.equal(out.service, SERVICE_INFO);
    assert.ok(out.paymentInfo);
    assert.ok(out.challenge.challengeId);
    assert.equal(out.credentialTemplate.protocol, MPP_PROTOCOL);
    assert.equal(out.credentialTemplate.challengeId, out.challenge.challengeId);
    assert.equal(out.credentialTemplate.payer, '<payer-id>');
    assert.deepEqual(out.retryExample, {
      jsonrpc: '2.0',
      id: 'req-9',
      method: 'create_order',
      params: { customerId: 'c1' },
      _meta: { payment: out.credentialTemplate },
    });
    assert.equal('inputSchema' in out, false);
  });
});
