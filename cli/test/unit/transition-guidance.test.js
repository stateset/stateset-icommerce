/**
 * Catalog guidance on refused and previewed status moves
 * (src/mcp/transition-guidance.js), on the refusal sentences the engine and
 * the kernel actually produce (each one below was captured from the real
 * binding), and its attachment to the tool result contract.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { guidanceForRefusal, previewForMove } from '../../src/mcp/transition-guidance.js';
import { withMovePreview } from '../../src/mcp/permission-gating.js';
import {
  normalizeStepOutcome,
  normalizeToolError,
  normalizeToolResult,
  validateToolResultContract,
} from '../../src/mcp/tool-result-contract.js';

describe('guidanceForRefusal', () => {
  it('reads a kernel refusal and names the tool for every allowed move', () => {
    const guidance = guidanceForRefusal({
      toolName: 'mark_return_received',
      message: 'return cannot transition from approved to received',
      code: 'commerce.invalid_return_status_transition',
    });
    assert.deepEqual(guidance.allowedNext, ['in_transit', 'cancelled']);
    assert.deepEqual(guidance.transition, { machine: 'return', from: 'approved', to: 'received' });
    assert.equal(
      guidance.hint,
      'A return in approved can move to: in_transit (add_return_tracking), cancelled ' +
        '(cancel_return). To reach received: approved -> in_transit -> received.',
    );
  });

  for (const [toolName, message, expected] of [
    [
      'update_order_status',
      'order cannot transition from cancelled to confirmed',
      { machine: 'order', from: 'cancelled', to: 'confirmed', allowedNext: [] },
    ],
    [
      'activate_product',
      'Failed to activate product: Validation error: product status cannot transition from archived to active',
      { machine: 'product', from: 'archived', to: 'active', allowedNext: [] },
    ],
    [
      'deliver_shipment',
      'Failed to deliver: Validation error: Invalid shipment status transition from pending to delivered',
      {
        machine: 'shipment',
        from: 'pending',
        to: 'delivered',
        allowedNext: ['processing', 'cancelled', 'on_hold'],
      },
    ],
    [
      'cancel_payment',
      'Failed to cancel payment: Conflict: Payment is completed and cannot transition to cancelled',
      {
        machine: 'payment',
        from: 'completed',
        to: 'cancelled',
        allowedNext: ['refunded', 'partially_refunded', 'disputed'],
      },
    ],
    [
      'cancel_cart',
      'Failed to cancel cart: Conflict: Cart cannot be cancelled in status: expired',
      { machine: 'cart', from: 'expired', to: 'cancelled', allowedNext: [] },
    ],
    [
      'begin_cart_checkout',
      'Failed to begin checkout: Conflict: Cart cannot begin checkout in status: expired',
      { machine: 'cart', from: 'expired', to: 'payment_pending', allowedNext: [] },
    ],
    [
      'create_refund',
      "Validation error: Cannot refund a payment in status 'pending'; only completed or partially refunded payments are refundable",
      {
        machine: 'payment',
        from: 'pending',
        allowedNext: ['processing', 'failed', 'cancelled', 'completed'],
      },
    ],
    [
      'update_customer',
      'Failed to update customer: Conflict: customer c952ab9b-ab58-405b-a65a-359392397970 is deleted and can no longer be updated',
      { machine: 'customer', from: 'deleted', allowedNext: [] },
    ],
    [
      'complete_payment',
      'Operation not permitted: payment is already completed',
      {
        machine: 'payment',
        from: 'completed',
        to: 'completed',
        allowedNext: ['refunded', 'partially_refunded', 'disputed'],
      },
    ],
  ]) {
    it(`reads ${toolName}: "${message.slice(0, 60)}..."`, () => {
      const guidance = guidanceForRefusal({ toolName, message });
      assert.ok(guidance, 'recognised');
      const { allowedNext, ...transition } = expected;
      assert.deepEqual(guidance.allowedNext, allowedNext);
      assert.equal(guidance.transition.machine, transition.machine);
      assert.equal(guidance.transition.from, transition.from);
      assert.equal(guidance.transition.to, transition.to);
      if (allowedNext.length === 0) assert.match(guidance.hint, /is final/);
    });
  }

  it('takes the target from the call when the refusal does not name it', () => {
    const guidance = guidanceForRefusal({
      toolName: 'update_customer',
      params: { status: 'active' },
      message: 'customer 1f is deleted and can no longer be updated',
    });
    assert.equal(guidance.transition.to, 'active');
  });

  it('points a shipping refusal at ship_order', () => {
    const guidance = guidanceForRefusal({
      toolName: 'update_order_status',
      params: { status: 'shipped' },
      message: 'shipment transitions must use orders.ship',
      code: 'commerce.shipment_command_required',
    });
    assert.equal(guidance.allowedNext, undefined);
    assert.deepEqual(guidance.transition, { machine: 'order', to: 'shipped' });
    assert.match(guidance.hint, /call ship_order/);
  });

  it('ignores failures that are not status moves', () => {
    for (const message of [
      'Order not found',
      'Payment link is already revoked',
      'Validation error: Cart is not ready for checkout',
      "Invalid parameters for tool 'get_order'",
    ]) {
      assert.equal(guidanceForRefusal({ toolName: 'cancel_cart', message }), null, message);
    }
    // A state name the machine does not have is not guessed at.
    assert.equal(
      guidanceForRefusal({ message: 'order cannot transition from limbo to shipped' }),
      null,
    );
  });
});

describe('previewForMove', () => {
  it('states where the engine accepts the move from, with the tool that gets there', () => {
    const preview = previewForMove('mark_return_received', { returnId: 'r1' });
    assert.deepEqual(preview.transition, {
      machine: 'return',
      to: 'received',
      acceptedFrom: ['in_transit'],
    });
    assert.match(
      preview.sentence,
      /only for a return in in_transit \(reached via add_return_tracking\)/,
    );
  });

  it('reads the target from a status parameter, and skips calls that are not moves', () => {
    assert.deepEqual(previewForMove('update_order_status', { status: 'refunded' }).transition, {
      machine: 'order',
      to: 'refunded',
      acceptedFrom: ['delivered'],
    });
    assert.equal(previewForMove('update_customer', { email: 'a@example.com' }), null);
    assert.equal(previewForMove('create_order', {}), null);
  });

  /**
   * The catalog lists every move into a status, not the moves a particular
   * tool makes. resume_subscription resumes only a paused subscription, but
   * the preview advertised trial, past_due and pending too -- every state
   * with a move into active.
   */
  it("names only the source states the tool itself accepts (its mapping's `from`)", () => {
    const resume = previewForMove('resume_subscription', { subscriptionId: 'sub-1' });
    assert.deepEqual(resume.transition, {
      machine: 'subscription',
      to: 'active',
      acceptedFrom: ['paused'],
    });
    assert.match(
      resume.sentence,
      /only for a subscription in paused \(reached via pause_subscription\);/,
    );
    assert.doesNotMatch(resume.sentence, /trial|past_due|pending/);

    // Same rule elsewhere: complete_payment does not settle a disputed
    // payment, approve_purchase_order does not approve one on hold.
    assert.deepEqual(
      previewForMove('complete_payment', { paymentId: 'p-1' }).transition.acceptedFrom,
      ['pending', 'processing', 'requires_action'],
    );
    assert.deepEqual(
      previewForMove('approve_purchase_order', { id: 'po-1' }).transition.acceptedFrom,
      ['pending_approval'],
    );
  });

  /**
   * cancel_subscription with `immediate: true` ends the subscription now and
   * the engine stores `expired`; only the default end-of-period cancellation
   * stores `cancelled`. The preview reported `cancelled` for both. Either is
   * accepted from every non-terminal status.
   */
  it('previews an immediate cancellation as expired and a default one as cancelled', () => {
    const nonTerminal = ['trial', 'active', 'paused', 'past_due', 'pending'];

    const immediate = previewForMove('cancel_subscription', {
      subscriptionId: 'sub-1',
      immediate: true,
    });
    assert.deepEqual(immediate.transition, {
      machine: 'subscription',
      to: 'expired',
      acceptedFrom: nonTerminal,
    });

    for (const params of [
      { subscriptionId: 'sub-1', immediate: false },
      { subscriptionId: 'sub-1' },
    ]) {
      assert.deepEqual(previewForMove('cancel_subscription', params).transition, {
        machine: 'subscription',
        to: 'cancelled',
        acceptedFrom: nonTerminal,
      });
    }

    const decision = (params) =>
      withMovePreview('cancel_subscription', params, {
        allowed: false,
        preview: true,
        reason: "Preview mode: would execute 'cancel_subscription' if --apply flag is set",
        wouldDo: { tool: 'cancel_subscription', params },
      });
    assert.equal(
      decision({ subscriptionId: 'sub-1', immediate: true }).wouldDo.transition.to,
      'expired',
    );
    assert.equal(decision({ subscriptionId: 'sub-1' }).wouldDo.transition.to, 'cancelled');
  });

  it('extends a preview decision without touching any other decision', () => {
    const preview = withMovePreview(
      'complete_return',
      { returnId: 'r1' },
      {
        allowed: false,
        preview: true,
        reason: "Preview mode: would execute 'complete_return' if --apply flag is set",
        wouldDo: { tool: 'complete_return', params: { returnId: 'r1' } },
      },
    );
    assert.match(preview.reason, /^Preview mode: would execute 'complete_return'/);
    assert.match(
      preview.reason,
      /only for a return in received \(reached via mark_return_received\) or inspecting;/,
    );
    assert.deepEqual(preview.wouldDo.transition.acceptedFrom, ['received', 'inspecting']);
    assert.deepEqual(preview.wouldDo.params, { returnId: 'r1' });

    const allowed = { allowed: true };
    assert.equal(withMovePreview('complete_return', {}, allowed), allowed);
    const other = { allowed: false, preview: true, reason: 'x' };
    assert.equal(withMovePreview('create_order', {}, other), other);
  });
});

describe('guidance on the tool result contract', () => {
  const receipt = {
    receipt_id: 'rcpt-1',
    command_type: 'returns.transition',
    status: 'rejected',
    error_code: 'commerce.invalid_return_status_transition',
    error_message: 'return cannot transition from approved to received',
    retry: 'never',
  };

  it('adds allowedNext to a rejected kernel receipt when the tool is known', () => {
    const outcome = normalizeToolResult(
      { success: false, kernel: true, receipt },
      { toolName: 'mark_return_received' },
    );
    assert.equal(outcome.ok, false);
    assert.equal(outcome.error.code, receipt.error_code);
    assert.deepEqual(outcome.error.allowedNext, ['in_transit', 'cancelled']);
    assert.match(outcome.error.hint, /add_return_tracking/);
    assert.deepEqual(validateToolResultContract(outcome), []);

    // Without the call, the contract is exactly what it was.
    const bare = normalizeToolResult({ success: false, kernel: true, receipt });
    assert.equal(bare.error.allowedNext, undefined);
    assert.equal(bare.error.hint, undefined);
  });

  it('adds it to a thrown engine error and to a step outcome', () => {
    const thrown = normalizeToolError(
      {
        code: 'CONFLICT',
        message: 'Failed to cancel cart: Conflict: Cart cannot be cancelled in status: expired',
      },
      { toolName: 'cancel_cart' },
    );
    assert.deepEqual(thrown.error.allowedNext, []);
    assert.equal(thrown.error.retryable, false);

    const step = normalizeStepOutcome({
      status: 'error',
      tool: 'mark_return_received',
      params: { returnId: 'r1' },
      result: { success: false, kernel: true, receipt },
    });
    assert.deepEqual(step.error.allowedNext, ['in_transit', 'cancelled']);
    assert.deepEqual(validateToolResultContract(step), []);
  });

  it('keeps an existing hint after the guidance', () => {
    const outcome = normalizeToolResult(
      { success: false, error: 'order cannot transition from pending to delivered', hint: 'x' },
      { toolName: 'update_order_status' },
    );
    assert.match(outcome.error.hint, /^An order in pending can move to: .* x$/);
  });
});
