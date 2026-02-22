/**
 * A2A Tools Zod Input Schema Validation Tests
 *
 * Tests all Zod constraints added to the A2A tools module:
 * - URL validators (.url())
 * - Array bounds (.min() / .max())
 * - Numeric constraints (.int(), .min(), .positive(), etc.)
 * - Limit fields (int, min 1, max 500)
 * - String constraints (.min() / .max())
 * - Enum constraints (.enum())
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';
import { a2aTools } from '../../src/tools/a2a.js';

const findTool = (name) => a2aTools.find((t) => t.name === name);

/** Build a z.object() from a tool's inputSchema and safeParse the data. */
const parse = (toolName, data) => {
  const tool = findTool(toolName);
  assert.ok(tool, `Tool ${toolName} not found`);
  const schema = z.object(tool.inputSchema);
  return schema.safeParse(data);
};

// ---------------------------------------------------------------------------
// URL validators
// ---------------------------------------------------------------------------
describe('A2A tools Zod validation', () => {
  describe('URL validators', () => {
    // -- a2a_request_payment: callbackUrl --
    it('a2a_request_payment accepts valid callbackUrl', () => {
      const result = parse('a2a_request_payment', {
        amount: 10,
        description: 'Test payment',
        callbackUrl: 'https://example.com/webhook',
      });
      assert.equal(result.success, true);
    });

    it('a2a_request_payment rejects invalid callbackUrl', () => {
      const result = parse('a2a_request_payment', {
        amount: 10,
        description: 'Test payment',
        callbackUrl: 'not-a-url',
      });
      assert.equal(result.success, false);
    });

    it('a2a_request_payment allows omitted callbackUrl', () => {
      const result = parse('a2a_request_payment', {
        amount: 10,
        description: 'Test payment',
      });
      assert.equal(result.success, true);
    });

    // -- a2a_register_service: endpointUrl --
    it('a2a_register_service accepts valid endpointUrl', () => {
      const result = parse('a2a_register_service', {
        name: 'My Service',
        description: 'A test service',
        category: 'api',
        pricingModel: 'fixed',
        endpointUrl: 'https://api.example.com/v1',
      });
      assert.equal(result.success, true);
    });

    it('a2a_register_service rejects invalid endpointUrl', () => {
      const result = parse('a2a_register_service', {
        name: 'My Service',
        description: 'A test service',
        category: 'api',
        pricingModel: 'fixed',
        endpointUrl: 'not-a-valid-url',
      });
      assert.equal(result.success, false);
    });

    it('a2a_register_service allows omitted endpointUrl', () => {
      const result = parse('a2a_register_service', {
        name: 'My Service',
        description: 'A test service',
        category: 'api',
        pricingModel: 'fixed',
      });
      assert.equal(result.success, true);
    });

    // -- a2a_send_notification: endpointUrl --
    it('a2a_send_notification accepts valid endpointUrl', () => {
      const result = parse('a2a_send_notification', {
        recipientAddress: '0xabc',
        eventType: 'payment.completed',
        payload: { id: '123' },
        endpointUrl: 'https://hooks.example.com/notify',
      });
      assert.equal(result.success, true);
    });

    it('a2a_send_notification rejects invalid endpointUrl', () => {
      const result = parse('a2a_send_notification', {
        recipientAddress: '0xabc',
        eventType: 'payment.completed',
        payload: { id: '123' },
        endpointUrl: 'not a url at all',
      });
      assert.equal(result.success, false);
    });

    // -- a2a_configure_webhooks: endpointUrl (required) --
    it('a2a_configure_webhooks accepts valid endpointUrl', () => {
      const result = parse('a2a_configure_webhooks', {
        agentAddress: '0xabc',
        endpointUrl: 'https://webhooks.example.com',
      });
      assert.equal(result.success, true);
    });

    it('a2a_configure_webhooks rejects invalid endpointUrl', () => {
      const result = parse('a2a_configure_webhooks', {
        agentAddress: '0xabc',
        endpointUrl: 'ht!tp://bad url',
      });
      assert.equal(result.success, false);
    });

    it('a2a_configure_webhooks rejects missing endpointUrl', () => {
      const result = parse('a2a_configure_webhooks', {
        agentAddress: '0xabc',
      });
      assert.equal(result.success, false);
    });
  });

  // ---------------------------------------------------------------------------
  // Array bounds
  // ---------------------------------------------------------------------------
  describe('Array bounds', () => {
    // -- a2a_request_quote: items min 1, max 100 --
    it('a2a_request_quote accepts 1 item', () => {
      const result = parse('a2a_request_quote', {
        seller: '0xseller',
        items: [{ description: 'Widget' }],
      });
      assert.equal(result.success, true);
    });

    it('a2a_request_quote rejects empty items array', () => {
      const result = parse('a2a_request_quote', {
        seller: '0xseller',
        items: [],
      });
      assert.equal(result.success, false);
    });

    it('a2a_request_quote rejects >100 items', () => {
      const items = Array.from({ length: 101 }, (_, i) => ({
        description: `Item ${i}`,
      }));
      const result = parse('a2a_request_quote', {
        seller: '0xseller',
        items,
      });
      assert.equal(result.success, false);
    });

    it('a2a_request_quote accepts exactly 100 items', () => {
      const items = Array.from({ length: 100 }, (_, i) => ({
        description: `Item ${i}`,
      }));
      const result = parse('a2a_request_quote', {
        seller: '0xseller',
        items,
      });
      assert.equal(result.success, true);
    });

    // -- a2a_configure_webhooks: enabledEvents max 50 --
    it('a2a_configure_webhooks accepts 50 enabledEvents', () => {
      const events = Array.from({ length: 50 }, (_, i) => `event.${i}`);
      const result = parse('a2a_configure_webhooks', {
        agentAddress: '0xabc',
        endpointUrl: 'https://example.com/hook',
        enabledEvents: events,
      });
      assert.equal(result.success, true);
    });

    it('a2a_configure_webhooks rejects >50 enabledEvents', () => {
      const events = Array.from({ length: 51 }, (_, i) => `event.${i}`);
      const result = parse('a2a_configure_webhooks', {
        agentAddress: '0xabc',
        endpointUrl: 'https://example.com/hook',
        enabledEvents: events,
      });
      assert.equal(result.success, false);
    });

    // -- a2a_subscribe_events: eventTypes max 50 --
    it('a2a_subscribe_events accepts 50 eventTypes', () => {
      const types = Array.from({ length: 50 }, (_, i) => `type.${i}`);
      const result = parse('a2a_subscribe_events', {
        agentAddress: '0xabc',
        eventTypes: types,
      });
      assert.equal(result.success, true);
    });

    it('a2a_subscribe_events rejects >50 eventTypes', () => {
      const types = Array.from({ length: 51 }, (_, i) => `type.${i}`);
      const result = parse('a2a_subscribe_events', {
        agentAddress: '0xabc',
        eventTypes: types,
      });
      assert.equal(result.success, false);
    });

    // -- a2a_get_event_history: eventTypes max 50 --
    it('a2a_get_event_history accepts 50 eventTypes', () => {
      const types = Array.from({ length: 50 }, (_, i) => `type.${i}`);
      const result = parse('a2a_get_event_history', {
        agentAddress: '0xabc',
        eventTypes: types,
      });
      assert.equal(result.success, true);
    });

    it('a2a_get_event_history rejects >50 eventTypes', () => {
      const types = Array.from({ length: 51 }, (_, i) => `type.${i}`);
      const result = parse('a2a_get_event_history', {
        agentAddress: '0xabc',
        eventTypes: types,
      });
      assert.equal(result.success, false);
    });

    // -- a2a_create_escrow: conditions max 20 --
    it('a2a_create_escrow accepts 20 conditions', () => {
      const conditions = Array.from({ length: 20 }, () => ({
        type: 'milestone',
        description: 'Step',
      }));
      const result = parse('a2a_create_escrow', {
        buyerAddress: '0xbuyer',
        sellerAddress: '0xseller',
        amount: 100,
        conditions,
      });
      assert.equal(result.success, true);
    });

    it('a2a_create_escrow rejects >20 conditions', () => {
      const conditions = Array.from({ length: 21 }, () => ({
        type: 'milestone',
        description: 'Step',
      }));
      const result = parse('a2a_create_escrow', {
        buyerAddress: '0xbuyer',
        sellerAddress: '0xseller',
        amount: 100,
        conditions,
      });
      assert.equal(result.success, false);
    });

    // -- a2a_create_conditional_payment: conditions max 20 --
    it('a2a_create_conditional_payment accepts 20 conditions', () => {
      const conditions = Array.from({ length: 20 }, () => ({
        type: 'buyer_confirmed',
      }));
      const result = parse('a2a_create_conditional_payment', {
        buyerAddress: '0xbuyer',
        sellerAddress: '0xseller',
        amount: 50,
        conditions,
      });
      assert.equal(result.success, true);
    });

    it('a2a_create_conditional_payment rejects >20 conditions', () => {
      const conditions = Array.from({ length: 21 }, () => ({
        type: 'buyer_confirmed',
      }));
      const result = parse('a2a_create_conditional_payment', {
        buyerAddress: '0xbuyer',
        sellerAddress: '0xseller',
        amount: 50,
        conditions,
      });
      assert.equal(result.success, false);
    });

    // -- a2a_create_split_payment: recipients min 2, max 20 --
    it('a2a_create_split_payment accepts 2 recipients', () => {
      const result = parse('a2a_create_split_payment', {
        senderAddress: '0xsender',
        totalAmount: 100,
        recipients: [
          { address: '0xa', percent: 50 },
          { address: '0xb', percent: 50 },
        ],
      });
      assert.equal(result.success, true);
    });

    it('a2a_create_split_payment rejects 1 recipient', () => {
      const result = parse('a2a_create_split_payment', {
        senderAddress: '0xsender',
        totalAmount: 100,
        recipients: [{ address: '0xa', percent: 100 }],
      });
      assert.equal(result.success, false);
    });

    it('a2a_create_split_payment rejects >20 recipients', () => {
      const recipients = Array.from({ length: 21 }, (_, i) => ({
        address: `0x${i}`,
        percent: 4,
      }));
      const result = parse('a2a_create_split_payment', {
        senderAddress: '0xsender',
        totalAmount: 100,
        recipients,
      });
      assert.equal(result.success, false);
    });

    it('a2a_create_split_payment accepts exactly 20 recipients', () => {
      const recipients = Array.from({ length: 20 }, (_, i) => ({
        address: `0x${i}`,
        percent: 5,
      }));
      const result = parse('a2a_create_split_payment', {
        senderAddress: '0xsender',
        totalAmount: 100,
        recipients,
      });
      assert.equal(result.success, true);
    });
  });

  // ---------------------------------------------------------------------------
  // Numeric constraints
  // ---------------------------------------------------------------------------
  describe('Numeric constraints', () => {
    // -- a2a_request_payment: expiresInHours int >= 1 --
    it('a2a_request_payment accepts expiresInHours=1', () => {
      const result = parse('a2a_request_payment', {
        amount: 10,
        description: 'Test',
        expiresInHours: 1,
      });
      assert.equal(result.success, true);
    });

    it('a2a_request_payment rejects expiresInHours=0', () => {
      const result = parse('a2a_request_payment', {
        amount: 10,
        description: 'Test',
        expiresInHours: 0,
      });
      assert.equal(result.success, false);
    });

    it('a2a_request_payment rejects non-integer expiresInHours', () => {
      const result = parse('a2a_request_payment', {
        amount: 10,
        description: 'Test',
        expiresInHours: 1.5,
      });
      assert.equal(result.success, false);
    });

    // -- a2a_provide_quote: fees >= 0, tax >= 0, expiresInHours int >= 1 --
    it('a2a_provide_quote accepts fees=0 and tax=0', () => {
      const result = parse('a2a_provide_quote', {
        quoteId: 'q-1',
        total: 100,
        fees: 0,
        tax: 0,
      });
      assert.equal(result.success, true);
    });

    it('a2a_provide_quote rejects negative fees', () => {
      const result = parse('a2a_provide_quote', {
        quoteId: 'q-1',
        total: 100,
        fees: -1,
      });
      assert.equal(result.success, false);
    });

    it('a2a_provide_quote rejects negative tax', () => {
      const result = parse('a2a_provide_quote', {
        quoteId: 'q-1',
        total: 100,
        tax: -0.01,
      });
      assert.equal(result.success, false);
    });

    it('a2a_provide_quote accepts expiresInHours=1', () => {
      const result = parse('a2a_provide_quote', {
        quoteId: 'q-1',
        total: 100,
        expiresInHours: 1,
      });
      assert.equal(result.success, true);
    });

    it('a2a_provide_quote rejects expiresInHours=0', () => {
      const result = parse('a2a_provide_quote', {
        quoteId: 'q-1',
        total: 100,
        expiresInHours: 0,
      });
      assert.equal(result.success, false);
    });

    // -- a2a_revise_quote: fees >= 0, tax >= 0 --
    it('a2a_revise_quote accepts fees=0', () => {
      const result = parse('a2a_revise_quote', {
        quoteId: 'q-1',
        total: 50,
        fees: 0,
      });
      assert.equal(result.success, true);
    });

    it('a2a_revise_quote rejects negative fees', () => {
      const result = parse('a2a_revise_quote', {
        quoteId: 'q-1',
        total: 50,
        fees: -5,
      });
      assert.equal(result.success, false);
    });

    it('a2a_revise_quote rejects negative tax', () => {
      const result = parse('a2a_revise_quote', {
        quoteId: 'q-1',
        total: 50,
        tax: -1,
      });
      assert.equal(result.success, false);
    });

    // -- a2a_create_escrow: expiresInHours int >= 1 --
    it('a2a_create_escrow accepts expiresInHours=1', () => {
      const result = parse('a2a_create_escrow', {
        buyerAddress: '0xbuyer',
        sellerAddress: '0xseller',
        amount: 100,
        expiresInHours: 1,
      });
      assert.equal(result.success, true);
    });

    it('a2a_create_escrow rejects expiresInHours=0', () => {
      const result = parse('a2a_create_escrow', {
        buyerAddress: '0xbuyer',
        sellerAddress: '0xseller',
        amount: 100,
        expiresInHours: 0,
      });
      assert.equal(result.success, false);
    });

    it('a2a_create_escrow rejects non-integer expiresInHours', () => {
      const result = parse('a2a_create_escrow', {
        buyerAddress: '0xbuyer',
        sellerAddress: '0xseller',
        amount: 100,
        expiresInHours: 2.5,
      });
      assert.equal(result.success, false);
    });

    // -- a2a_create_conditional_payment: expiresInHours int >= 1 --
    it('a2a_create_conditional_payment accepts expiresInHours=1', () => {
      const result = parse('a2a_create_conditional_payment', {
        buyerAddress: '0xbuyer',
        sellerAddress: '0xseller',
        amount: 50,
        expiresInHours: 1,
      });
      assert.equal(result.success, true);
    });

    it('a2a_create_conditional_payment rejects expiresInHours=0', () => {
      const result = parse('a2a_create_conditional_payment', {
        buyerAddress: '0xbuyer',
        sellerAddress: '0xseller',
        amount: 50,
        expiresInHours: 0,
      });
      assert.equal(result.success, false);
    });

    // -- a2a_resolve_dispute: amount positive when provided --
    it('a2a_resolve_dispute accepts positive amount', () => {
      const result = parse('a2a_resolve_dispute', {
        disputeId: 'd-1',
        resolutionType: 'partial_refund',
        amount: 25,
      });
      assert.equal(result.success, true);
    });

    it('a2a_resolve_dispute rejects zero amount', () => {
      const result = parse('a2a_resolve_dispute', {
        disputeId: 'd-1',
        resolutionType: 'partial_refund',
        amount: 0,
      });
      assert.equal(result.success, false);
    });

    it('a2a_resolve_dispute rejects negative amount', () => {
      const result = parse('a2a_resolve_dispute', {
        disputeId: 'd-1',
        resolutionType: 'partial_refund',
        amount: -10,
      });
      assert.equal(result.success, false);
    });

    it('a2a_resolve_dispute allows omitted amount', () => {
      const result = parse('a2a_resolve_dispute', {
        disputeId: 'd-1',
        resolutionType: 'full_refund',
      });
      assert.equal(result.success, true);
    });

    // -- a2a_create_agent_subscription: trialDays int >= 0, maxPastDueCycles int >= 0 --
    it('a2a_create_agent_subscription accepts trialDays=0', () => {
      const result = parse('a2a_create_agent_subscription', {
        subscriberAddress: '0xsub',
        providerAddress: '0xprov',
        planName: 'Pro',
        amount: 49.99,
        trialDays: 0,
      });
      assert.equal(result.success, true);
    });

    it('a2a_create_agent_subscription rejects negative trialDays', () => {
      const result = parse('a2a_create_agent_subscription', {
        subscriberAddress: '0xsub',
        providerAddress: '0xprov',
        planName: 'Pro',
        amount: 49.99,
        trialDays: -1,
      });
      assert.equal(result.success, false);
    });

    it('a2a_create_agent_subscription rejects non-integer trialDays', () => {
      const result = parse('a2a_create_agent_subscription', {
        subscriberAddress: '0xsub',
        providerAddress: '0xprov',
        planName: 'Pro',
        amount: 49.99,
        trialDays: 7.5,
      });
      assert.equal(result.success, false);
    });

    it('a2a_create_agent_subscription accepts maxPastDueCycles=0', () => {
      const result = parse('a2a_create_agent_subscription', {
        subscriberAddress: '0xsub',
        providerAddress: '0xprov',
        planName: 'Pro',
        amount: 49.99,
        maxPastDueCycles: 0,
      });
      assert.equal(result.success, true);
    });

    it('a2a_create_agent_subscription rejects negative maxPastDueCycles', () => {
      const result = parse('a2a_create_agent_subscription', {
        subscriberAddress: '0xsub',
        providerAddress: '0xprov',
        planName: 'Pro',
        amount: 49.99,
        maxPastDueCycles: -1,
      });
      assert.equal(result.success, false);
    });

    // -- a2a_create_split_payment: platformFeePercent 0-100, recipient percent 0-100, recipient amount positive --
    it('a2a_create_split_payment accepts platformFeePercent=0', () => {
      const result = parse('a2a_create_split_payment', {
        senderAddress: '0xsender',
        totalAmount: 100,
        recipients: [
          { address: '0xa', percent: 50 },
          { address: '0xb', percent: 50 },
        ],
        platformFeePercent: 0,
      });
      assert.equal(result.success, true);
    });

    it('a2a_create_split_payment accepts platformFeePercent=100', () => {
      const result = parse('a2a_create_split_payment', {
        senderAddress: '0xsender',
        totalAmount: 100,
        recipients: [
          { address: '0xa', percent: 50 },
          { address: '0xb', percent: 50 },
        ],
        platformFeePercent: 100,
      });
      assert.equal(result.success, true);
    });

    it('a2a_create_split_payment rejects platformFeePercent > 100', () => {
      const result = parse('a2a_create_split_payment', {
        senderAddress: '0xsender',
        totalAmount: 100,
        recipients: [
          { address: '0xa', percent: 50 },
          { address: '0xb', percent: 50 },
        ],
        platformFeePercent: 101,
      });
      assert.equal(result.success, false);
    });

    it('a2a_create_split_payment rejects negative platformFeePercent', () => {
      const result = parse('a2a_create_split_payment', {
        senderAddress: '0xsender',
        totalAmount: 100,
        recipients: [
          { address: '0xa', percent: 50 },
          { address: '0xb', percent: 50 },
        ],
        platformFeePercent: -1,
      });
      assert.equal(result.success, false);
    });

    it('a2a_create_split_payment accepts recipient percent=0', () => {
      const result = parse('a2a_create_split_payment', {
        senderAddress: '0xsender',
        totalAmount: 100,
        recipients: [
          { address: '0xa', percent: 0 },
          { address: '0xb', percent: 100 },
        ],
      });
      assert.equal(result.success, true);
    });

    it('a2a_create_split_payment rejects recipient percent > 100', () => {
      const result = parse('a2a_create_split_payment', {
        senderAddress: '0xsender',
        totalAmount: 100,
        recipients: [
          { address: '0xa', percent: 101 },
          { address: '0xb', percent: 50 },
        ],
      });
      assert.equal(result.success, false);
    });

    it('a2a_create_split_payment accepts positive recipient amount', () => {
      const result = parse('a2a_create_split_payment', {
        senderAddress: '0xsender',
        totalAmount: 100,
        splitType: 'fixed',
        recipients: [
          { address: '0xa', amount: 60 },
          { address: '0xb', amount: 40 },
        ],
      });
      assert.equal(result.success, true);
    });

    it('a2a_create_split_payment rejects zero recipient amount', () => {
      const result = parse('a2a_create_split_payment', {
        senderAddress: '0xsender',
        totalAmount: 100,
        splitType: 'fixed',
        recipients: [
          { address: '0xa', amount: 0 },
          { address: '0xb', amount: 100 },
        ],
      });
      assert.equal(result.success, false);
    });

    it('a2a_create_split_payment rejects negative recipient amount', () => {
      const result = parse('a2a_create_split_payment', {
        senderAddress: '0xsender',
        totalAmount: 100,
        splitType: 'fixed',
        recipients: [
          { address: '0xa', amount: -10 },
          { address: '0xb', amount: 110 },
        ],
      });
      assert.equal(result.success, false);
    });
  });

  // ---------------------------------------------------------------------------
  // Limit fields (int, min 1, max 500)
  // ---------------------------------------------------------------------------
  describe('Limit fields', () => {
    const limitTools = [
      'a2a_list_payments',
      'a2a_list_payment_requests',
      'a2a_list_quotes',
      'a2a_list_escrows',
      'a2a_list_disputes',
      'a2a_list_services',
      'a2a_list_notification_log',
      'a2a_list_agent_subscriptions',
      'a2a_list_split_payments',
      'a2a_get_event_history',
    ];

    /** Build minimal valid data for each limit tool. */
    const minData = (toolName) => {
      if (toolName === 'a2a_get_event_history') {
        return { agentAddress: '0xabc' };
      }
      return {};
    };

    for (const toolName of limitTools) {
      it(`${toolName} accepts limit=1`, () => {
        const result = parse(toolName, { ...minData(toolName), limit: 1 });
        assert.equal(result.success, true);
      });

      it(`${toolName} accepts limit=500`, () => {
        const result = parse(toolName, { ...minData(toolName), limit: 500 });
        assert.equal(result.success, true);
      });

      it(`${toolName} rejects limit=0`, () => {
        const result = parse(toolName, { ...minData(toolName), limit: 0 });
        assert.equal(result.success, false);
      });

      it(`${toolName} rejects limit=501`, () => {
        const result = parse(toolName, { ...minData(toolName), limit: 501 });
        assert.equal(result.success, false);
      });

      it(`${toolName} rejects non-integer limit`, () => {
        const result = parse(toolName, { ...minData(toolName), limit: 10.5 });
        assert.equal(result.success, false);
      });

      it(`${toolName} allows omitted limit`, () => {
        const result = parse(toolName, { ...minData(toolName) });
        assert.equal(result.success, true);
      });
    }
  });

  // ---------------------------------------------------------------------------
  // String constraints
  // ---------------------------------------------------------------------------
  describe('String constraints', () => {
    // -- a2a_dispute_escrow: reason min 1, max 500 --
    it('a2a_dispute_escrow accepts reason with 1 char', () => {
      const result = parse('a2a_dispute_escrow', {
        escrowId: 'e-1',
        reason: 'X',
      });
      assert.equal(result.success, true);
    });

    it('a2a_dispute_escrow accepts reason with 500 chars', () => {
      const result = parse('a2a_dispute_escrow', {
        escrowId: 'e-1',
        reason: 'A'.repeat(500),
      });
      assert.equal(result.success, true);
    });

    it('a2a_dispute_escrow rejects empty reason', () => {
      const result = parse('a2a_dispute_escrow', {
        escrowId: 'e-1',
        reason: '',
      });
      assert.equal(result.success, false);
    });

    it('a2a_dispute_escrow rejects reason >500 chars', () => {
      const result = parse('a2a_dispute_escrow', {
        escrowId: 'e-1',
        reason: 'A'.repeat(501),
      });
      assert.equal(result.success, false);
    });

    // -- a2a_rate_agent: comment max 1000 --
    it('a2a_rate_agent accepts comment with 1000 chars', () => {
      const result = parse('a2a_rate_agent', {
        agentAddress: '0xagent',
        transactionType: 'payment',
        transactionId: 'tx-1',
        score: 5,
        comment: 'B'.repeat(1000),
      });
      assert.equal(result.success, true);
    });

    it('a2a_rate_agent rejects comment >1000 chars', () => {
      const result = parse('a2a_rate_agent', {
        agentAddress: '0xagent',
        transactionType: 'payment',
        transactionId: 'tx-1',
        score: 5,
        comment: 'B'.repeat(1001),
      });
      assert.equal(result.success, false);
    });

    it('a2a_rate_agent allows omitted comment', () => {
      const result = parse('a2a_rate_agent', {
        agentAddress: '0xagent',
        transactionType: 'payment',
        transactionId: 'tx-1',
        score: 5,
      });
      assert.equal(result.success, true);
    });

    // -- a2a_respond_to_feedback: response min 1, max 2000 --
    it('a2a_respond_to_feedback accepts response with 1 char', () => {
      const result = parse('a2a_respond_to_feedback', {
        feedbackId: 'f-1',
        response: 'Y',
      });
      assert.equal(result.success, true);
    });

    it('a2a_respond_to_feedback accepts response with 2000 chars', () => {
      const result = parse('a2a_respond_to_feedback', {
        feedbackId: 'f-1',
        response: 'C'.repeat(2000),
      });
      assert.equal(result.success, true);
    });

    it('a2a_respond_to_feedback rejects empty response', () => {
      const result = parse('a2a_respond_to_feedback', {
        feedbackId: 'f-1',
        response: '',
      });
      assert.equal(result.success, false);
    });

    it('a2a_respond_to_feedback rejects response >2000 chars', () => {
      const result = parse('a2a_respond_to_feedback', {
        feedbackId: 'f-1',
        response: 'C'.repeat(2001),
      });
      assert.equal(result.success, false);
    });

    // -- a2a_request_quote: item quantity int >= 1 --
    it('a2a_request_quote accepts item quantity=1', () => {
      const result = parse('a2a_request_quote', {
        seller: '0xseller',
        items: [{ description: 'Widget', quantity: 1 }],
      });
      assert.equal(result.success, true);
    });

    it('a2a_request_quote rejects item quantity=0', () => {
      const result = parse('a2a_request_quote', {
        seller: '0xseller',
        items: [{ description: 'Widget', quantity: 0 }],
      });
      assert.equal(result.success, false);
    });

    it('a2a_request_quote rejects non-integer item quantity', () => {
      const result = parse('a2a_request_quote', {
        seller: '0xseller',
        items: [{ description: 'Widget', quantity: 1.5 }],
      });
      assert.equal(result.success, false);
    });
  });

  // ---------------------------------------------------------------------------
  // Enum constraints
  // ---------------------------------------------------------------------------
  describe('Enum constraints', () => {
    // -- a2a_dispute_escrow: category enum --
    const validCategories = [
      'non_delivery',
      'poor_quality',
      'not_as_described',
      'overcharged',
      'unauthorized',
      'other',
    ];

    for (const cat of validCategories) {
      it(`a2a_dispute_escrow accepts category="${cat}"`, () => {
        const result = parse('a2a_dispute_escrow', {
          escrowId: 'e-1',
          reason: 'Dispute reason',
          category: cat,
        });
        assert.equal(result.success, true);
      });
    }

    it('a2a_dispute_escrow rejects invalid category', () => {
      const result = parse('a2a_dispute_escrow', {
        escrowId: 'e-1',
        reason: 'Dispute reason',
        category: 'invalid_category',
      });
      assert.equal(result.success, false);
    });

    it('a2a_dispute_escrow allows omitted category', () => {
      const result = parse('a2a_dispute_escrow', {
        escrowId: 'e-1',
        reason: 'Dispute reason',
      });
      assert.equal(result.success, true);
    });

    // -- a2a_resolve_dispute: resolutionType enum --
    const validResolutions = [
      'full_refund',
      'partial_refund',
      'release_to_seller',
      'split',
      'escalated',
    ];

    for (const rt of validResolutions) {
      it(`a2a_resolve_dispute accepts resolutionType="${rt}"`, () => {
        const result = parse('a2a_resolve_dispute', {
          disputeId: 'd-1',
          resolutionType: rt,
        });
        assert.equal(result.success, true);
      });
    }

    it('a2a_resolve_dispute rejects invalid resolutionType', () => {
      const result = parse('a2a_resolve_dispute', {
        disputeId: 'd-1',
        resolutionType: 'magical_resolution',
      });
      assert.equal(result.success, false);
    });

    // -- a2a_rate_agent: transactionType enum --
    const validTxTypes = ['quote', 'payment', 'escrow', 'service'];

    for (const tt of validTxTypes) {
      it(`a2a_rate_agent accepts transactionType="${tt}"`, () => {
        const result = parse('a2a_rate_agent', {
          agentAddress: '0xagent',
          transactionType: tt,
          transactionId: 'tx-1',
          score: 3,
        });
        assert.equal(result.success, true);
      });
    }

    it('a2a_rate_agent rejects invalid transactionType', () => {
      const result = parse('a2a_rate_agent', {
        agentAddress: '0xagent',
        transactionType: 'barter',
        transactionId: 'tx-1',
        score: 3,
      });
      assert.equal(result.success, false);
    });

    // -- a2a_rate_agent: score int min 1 max 5 --
    it('a2a_rate_agent accepts score=1', () => {
      const result = parse('a2a_rate_agent', {
        agentAddress: '0xagent',
        transactionType: 'payment',
        transactionId: 'tx-1',
        score: 1,
      });
      assert.equal(result.success, true);
    });

    it('a2a_rate_agent accepts score=5', () => {
      const result = parse('a2a_rate_agent', {
        agentAddress: '0xagent',
        transactionType: 'payment',
        transactionId: 'tx-1',
        score: 5,
      });
      assert.equal(result.success, true);
    });

    it('a2a_rate_agent rejects score=0', () => {
      const result = parse('a2a_rate_agent', {
        agentAddress: '0xagent',
        transactionType: 'payment',
        transactionId: 'tx-1',
        score: 0,
      });
      assert.equal(result.success, false);
    });

    it('a2a_rate_agent rejects score=6', () => {
      const result = parse('a2a_rate_agent', {
        agentAddress: '0xagent',
        transactionType: 'payment',
        transactionId: 'tx-1',
        score: 6,
      });
      assert.equal(result.success, false);
    });

    // -- a2a_create_agent_subscription: billingInterval enum --
    const validIntervals = ['weekly', 'biweekly', 'monthly', 'quarterly', 'annual'];

    for (const interval of validIntervals) {
      it(`a2a_create_agent_subscription accepts billingInterval="${interval}"`, () => {
        const result = parse('a2a_create_agent_subscription', {
          subscriberAddress: '0xsub',
          providerAddress: '0xprov',
          planName: 'Plan',
          amount: 10,
          billingInterval: interval,
        });
        assert.equal(result.success, true);
      });
    }

    it('a2a_create_agent_subscription rejects invalid billingInterval', () => {
      const result = parse('a2a_create_agent_subscription', {
        subscriberAddress: '0xsub',
        providerAddress: '0xprov',
        planName: 'Plan',
        amount: 10,
        billingInterval: 'daily',
      });
      assert.equal(result.success, false);
    });

    // -- a2a_register_service: category and pricingModel enums --
    it('a2a_register_service rejects invalid category', () => {
      const result = parse('a2a_register_service', {
        name: 'Svc',
        description: 'Desc',
        category: 'invalid',
        pricingModel: 'fixed',
      });
      assert.equal(result.success, false);
    });

    it('a2a_register_service rejects invalid pricingModel', () => {
      const result = parse('a2a_register_service', {
        name: 'Svc',
        description: 'Desc',
        category: 'api',
        pricingModel: 'auction',
      });
      assert.equal(result.success, false);
    });
  });
});
