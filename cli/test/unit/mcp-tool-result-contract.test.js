/**
 * The tool result contract (src/mcp/tool-result-contract.js): every shape a
 * tool can answer with normalizes to `{ ok, preview, result?, error?, notice? }`.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  ToolErrorCode,
  ToolNoticeCode,
  attachContractToResponse,
  isRetryable,
  normalizeStepOutcome,
  normalizeToolError,
  normalizeToolResult,
  toToolResultContract,
  validateToolResultContract,
} from '../../src/mcp/tool-result-contract.js';
import { applyRequired } from '../../src/utils/apply-guard.js';

const valid = (outcome) => {
  assert.deepEqual(validateToolResultContract(outcome), [], JSON.stringify(outcome));
  return outcome;
};

const receipt = (overrides = {}) => ({
  receipt_id: 'r-1',
  command_type: 'orders.ship',
  status: 'succeeded',
  error_code: null,
  error_message: null,
  retry: 'never',
  result: { id: 'o-1' },
  ...overrides,
});

describe('normalizeToolResult', () => {
  it('passes a successful result through untouched, by reference', () => {
    const result = { success: true, order: { id: 'o-1', totalAmountExact: '10.10' } };
    const outcome = valid(normalizeToolResult(result));
    assert.equal(outcome.ok, true);
    assert.equal(outcome.preview, false);
    assert.equal(outcome.result, result);
    assert.equal(outcome.error, undefined);
  });

  it('treats non-object results as successful values', () => {
    for (const value of [null, 'text', 3, [1, 2]]) {
      const outcome = valid(normalizeToolResult(value));
      assert.equal(outcome.ok, true);
    }
  });

  it('fails { success: false, error } and keeps the result', () => {
    const result = { success: false, error: 'Order not found' };
    const outcome = valid(normalizeToolResult(result));
    assert.equal(outcome.ok, false);
    assert.equal(outcome.error.code, ToolErrorCode.NOT_FOUND);
    assert.equal(outcome.error.message, 'Order not found');
    assert.equal(outcome.error.retryable, false);
    assert.equal(outcome.result, result);
  });

  it('fails a bare truthy error without a success flag', () => {
    const outcome = valid(normalizeToolResult({ error: 'something broke' }));
    assert.equal(outcome.ok, false);
    assert.equal(outcome.error.code, ToolErrorCode.TOOL_ERROR);
  });

  it('lets an explicit success: true win over a stray error field', () => {
    assert.equal(normalizeToolResult({ success: true, error: null }).ok, true);
    assert.equal(normalizeToolResult({ success: true, error: 'warn' }).ok, true);
  });

  it('prefers a structured code over the message', () => {
    const outcome = valid(
      normalizeToolResult({
        success: false,
        error: { code: 'INSUFFICIENT_STOCK', message: 'not found either' },
      }),
    );
    assert.equal(outcome.error.code, 'INSUFFICIENT_STOCK');
    assert.equal(outcome.error.message, 'not found either');
    assert.equal(
      normalizeToolResult({ success: false, error: 'x', reasonCode: 'COUPON_EXPIRED' }).error.code,
      'COUPON_EXPIRED',
    );
  });

  it('never reads result.code, which is as likely a coupon code as an error code', () => {
    const outcome = normalizeToolResult({
      success: false,
      error: 'Coupon expired',
      code: 'SAVE10',
    });
    assert.equal(outcome.error.code, ToolErrorCode.TOOL_ERROR);
  });

  it('carries hint and remediation', () => {
    assert.equal(
      normalizeToolResult({ success: false, error: 'x', hint: 'do y' }).error.hint,
      'do y',
    );
    assert.equal(
      normalizeToolResult({ success: false, error: 'x', remediation: 'do z' }).error.hint,
      'do z',
    );
  });

  it('turns an --apply refusal into a preview, not an error', () => {
    const outcome = valid(normalizeToolResult(applyRequired('Create supplier', { name: 'n' })));
    assert.equal(outcome.ok, true);
    assert.equal(outcome.preview, true);
    assert.equal(outcome.notice.code, ToolNoticeCode.APPLY_REQUIRED);
    assert.deepEqual(outcome.result.wouldDo, { name: 'n' });

    const legacy = valid(
      normalizeToolResult({
        success: false,
        error: 'Create operation not allowed. The --apply flag must be set.',
      }),
    );
    assert.equal(legacy.ok, true);
    assert.equal(legacy.preview, true);
  });

  it('reads the permission-gate preview payload as a preview', () => {
    const outcome = valid(
      normalizeToolResult({
        error: "Preview mode: would execute 'x' if --apply flag is set",
        preview: true,
        wouldDo: { tool: 'x' },
      }),
    );
    assert.equal(outcome.ok, true);
    assert.equal(outcome.preview, true);
  });

  describe('kernel receipts', () => {
    it('succeeds on a succeeded receipt', () => {
      const outcome = valid(
        normalizeToolResult({ success: true, kernel: true, receipt: receipt(), result: {} }),
      );
      assert.equal(outcome.ok, true);
      assert.equal(outcome.preview, false);
    });

    it('fails a rejected receipt with its own error_code and message', () => {
      const result = {
        success: false,
        kernel: true,
        receipt: receipt({
          status: 'rejected',
          error_code: 'commerce.invalid_order_status_transition',
          error_message: 'order cannot transition from confirmed to shipped',
        }),
        result: null,
      };
      const outcome = valid(normalizeToolResult(result));
      assert.equal(outcome.ok, false);
      assert.equal(outcome.error.code, 'commerce.invalid_order_status_transition');
      assert.equal(outcome.error.message, 'order cannot transition from confirmed to shipped');
      assert.equal(outcome.error.retryable, false);
      assert.equal(outcome.result, result);
    });

    it('fails a rejected receipt even when the wrapper claims success', () => {
      const outcome = normalizeToolResult({
        success: true,
        receipt: receipt({ status: 'rejected', error_code: 'kernel.budget_exceeded' }),
      });
      assert.equal(outcome.ok, false);
      assert.equal(outcome.error.code, 'kernel.budget_exceeded');
    });

    it('falls back to KERNEL_REJECTED / KERNEL_FAILED without an error_code', () => {
      assert.equal(
        normalizeToolResult({ receipt: receipt({ status: 'rejected' }) }).error.code,
        ToolErrorCode.KERNEL_REJECTED,
      );
      assert.equal(
        normalizeToolResult({ receipt: receipt({ status: 'failed' }) }).error.code,
        ToolErrorCode.KERNEL_FAILED,
      );
    });

    it('maps the retry disposition to retryable and a hint', () => {
      const conflict = normalizeToolResult({
        receipt: receipt({
          status: 'rejected',
          error_code: 'commerce.checkout.conflict',
          retry: 'after_conflict',
        }),
      });
      assert.equal(conflict.error.retryable, true);
      assert.match(conflict.error.hint, /Reload/);
      const never = normalizeToolResult({
        receipt: receipt({ status: 'rejected', error_code: 'x.y', retry: 'never' }),
      });
      assert.equal(never.error.retryable, false);
      assert.equal(never.error.hint, undefined);
    });

    it('marks a previewed receipt as an ok preview', () => {
      const outcome = valid(
        normalizeToolResult({
          success: true,
          kernel: true,
          preview: true,
          receipt: receipt({ status: 'previewed' }),
        }),
      );
      assert.equal(outcome.ok, true);
      assert.equal(outcome.preview, true);
      assert.equal(outcome.notice.code, ToolNoticeCode.KERNEL_PREVIEW);
    });

    it('judges a bare receipt returned as the whole result', () => {
      const outcome = normalizeToolResult(
        receipt({ status: 'rejected', error_code: 'kernel.policy_denied' }),
      );
      assert.equal(outcome.ok, false);
      assert.equal(outcome.error.code, 'kernel.policy_denied');
    });
  });

  it('never introduces a number: exact money strings stay strings', () => {
    const result = { success: true, amountExact: '0.1', grandTotal: '19.99', nested: ['1.10'] };
    const outcome = normalizeToolResult(result);
    assert.equal(JSON.stringify(outcome.result), JSON.stringify(result));
    assert.equal(typeof outcome.result.amountExact, 'string');
  });
});

describe('normalizeToolError', () => {
  it('keeps a binding error code', () => {
    const error = Object.assign(new Error('Cart not found'), { code: 'NOT_FOUND' });
    const outcome = valid(normalizeToolError(error));
    assert.equal(outcome.ok, false);
    assert.equal(outcome.error.code, 'NOT_FOUND');
    assert.equal(outcome.error.message, 'Cart not found');
  });

  it('ignores napi statuses and unwraps an undecorated envelope', () => {
    const error = Object.assign(
      new Error(
        '{"code":"CONFLICT","message":"Version conflict on order o-1: expected version 3","details":{"expectedVersion":3}}',
      ),
      { code: 'GenericFailure' },
    );
    const outcome = valid(normalizeToolError(error));
    assert.equal(outcome.error.code, 'CONFLICT');
    assert.equal(outcome.error.message, 'Version conflict on order o-1: expected version 3');
    assert.equal(outcome.error.retryable, true);
  });

  it('does not retry a duplicate-key conflict', () => {
    const error = Object.assign(new Error('Duplicate SKU: W-1'), { code: 'CONFLICT' });
    assert.equal(normalizeToolError(error).error.retryable, false);
  });

  it('maps zod errors to INVALID_INPUT and unknown errors to TOOL_ERROR', () => {
    const zodLike = Object.assign(new Error('bad'), { name: 'ZodError', issues: [] });
    assert.equal(normalizeToolError(zodLike).error.code, ToolErrorCode.INVALID_INPUT);
    assert.equal(normalizeToolError(new Error('kaboom')).error.code, ToolErrorCode.TOOL_ERROR);
    assert.equal(normalizeToolError('plain string').error.message, 'plain string');
  });

  it('retries timeouts and rate limits only', () => {
    assert.equal(normalizeToolError(new Error('request timed out')).error.retryable, true);
    assert.equal(normalizeToolError(new Error('rate limit exceeded')).error.code, 'RATE_LIMITED');
    assert.equal(normalizeToolError(new Error('rate limit exceeded')).error.retryable, true);
    assert.equal(isRetryable('VALIDATION', 'x'), false);
    assert.equal(isRetryable('DATABASE', 'database is locked'), true);
    assert.equal(isRetryable('DATABASE', 'disk I/O error'), false);
  });
});

describe('normalizeStepOutcome', () => {
  it('maps every gate status to its code', () => {
    const cases = {
      blocked: ToolErrorCode.HOOK_BLOCKED,
      policy_block: ToolErrorCode.POLICY_DENIED,
      permission_block: ToolErrorCode.PERMISSION_DENIED,
      payment_required: ToolErrorCode.PAYMENT_REQUIRED,
      treasury_block: ToolErrorCode.TREASURY_BLOCKED,
    };
    for (const [status, code] of Object.entries(cases)) {
      const outcome = valid(normalizeStepOutcome({ status, error: `${status} happened` }));
      assert.equal(outcome.ok, false, status);
      assert.equal(outcome.error.code, code, status);
      assert.equal(outcome.error.message, `${status} happened`);
    }
  });

  it('reports schema-invalid input as INVALID_INPUT with the issues as a hint', () => {
    const outcome = valid(
      normalizeStepOutcome({
        status: 'invalid',
        error: "Invalid parameters for tool 'get_order'",
        notes: { validation: [{ path: 'identifier', message: 'Required' }] },
      }),
    );
    assert.equal(outcome.error.code, ToolErrorCode.INVALID_INPUT);
    assert.equal(outcome.error.hint, 'Fix the input: identifier: Required');
  });

  it('reports an unknown tool as UNKNOWN_TOOL', () => {
    const outcome = normalizeStepOutcome({ status: 'invalid', error: "Unknown tool 'nope'" });
    assert.equal(outcome.error.code, ToolErrorCode.UNKNOWN_TOOL);
  });

  it('keeps previews ok and flagged', () => {
    const outcome = valid(
      normalizeStepOutcome({
        status: 'preview',
        permission: { preview: true, reason: 'Preview mode' },
        wouldDo: { tool: 'x' },
        result: null,
      }),
    );
    assert.equal(outcome.ok, true);
    assert.equal(outcome.preview, true);
    assert.equal(outcome.notice.code, ToolNoticeCode.APPLY_REQUIRED);
    assert.deepEqual(outcome.result, { wouldDo: { tool: 'x' } });

    const dry = valid(
      normalizeStepOutcome({ status: 'dry_run_success', result: { dryRun: true } }),
    );
    assert.equal(dry.ok, true);
    assert.equal(dry.preview, true);
    assert.equal(dry.notice.code, ToolNoticeCode.DRY_RUN);
  });

  it('fails a governed preview whose receipt says the command would be rejected', () => {
    const outcome = valid(
      normalizeStepOutcome({
        status: 'preview',
        permission: { preview: true },
        result: { receipt: receipt({ status: 'rejected', error_code: 'kernel.policy_denied' }) },
      }),
    );
    assert.equal(outcome.ok, false);
    assert.equal(outcome.preview, true);
    assert.equal(outcome.error.code, 'kernel.policy_denied');
  });

  it('treats a treasury block during a dry run as a failure', () => {
    const outcome = valid(
      normalizeStepOutcome({ status: 'dry_run_blocked', error: 'over budget', permission: {} }),
    );
    assert.equal(outcome.ok, false);
    assert.equal(outcome.error.code, ToolErrorCode.TREASURY_BLOCKED);
  });

  it('judges a success status by the raw result, not the compacted copy', () => {
    const raw = { receipt: receipt({ status: 'rejected', error_code: 'a.b' }) };
    const outcome = normalizeStepOutcome({ status: 'success', result: { compacted: true } }, raw);
    assert.equal(outcome.ok, false);
    assert.equal(outcome.error.code, 'a.b');
  });

  it('keeps a thrown error code on error status', () => {
    const outcome = valid(
      normalizeStepOutcome({ status: 'error', error: 'Cart not found', errorCode: 'NOT_FOUND' }),
    );
    assert.equal(outcome.error.code, 'NOT_FOUND');
  });
});

describe('validateToolResultContract', () => {
  it('rejects malformed outcomes', () => {
    assert.ok(validateToolResultContract(null).length > 0);
    assert.ok(validateToolResultContract({ ok: false, preview: false }).length > 0);
    assert.ok(
      validateToolResultContract({
        ok: true,
        preview: false,
        error: { code: 'X', message: '', retryable: false },
      }).length > 0,
    );
    assert.ok(validateToolResultContract({ ok: true, preview: false, extra: 1 }).length > 0);
    assert.ok(
      validateToolResultContract({ ok: true, preview: false, notice: { code: 'X' } }).length > 0,
    );
  });
});

describe('attachContractToResponse / toToolResultContract', () => {
  it('sets isError exactly when ok is false and keeps content', () => {
    const content = [{ type: 'text', text: '{}' }];
    const failed = attachContractToResponse(
      { content, isError: false },
      normalizeToolResult({ success: false, error: 'x' }),
    );
    assert.equal(failed.isError, true);
    assert.equal(failed.content, content);
    const ok = attachContractToResponse({ content, isError: true }, normalizeToolResult({}));
    assert.equal(ok.isError, undefined);
    assert.equal(ok.structuredContent.ok, true);
  });

  it('recovers the contract from an executeTool return value', () => {
    const outcome = valid(
      toToolResultContract({
        success: false,
        ok: false,
        preview: false,
        result: null,
        error: 'x',
        failure: { code: 'NOT_FOUND', message: 'x', retryable: false },
      }),
    );
    assert.equal(outcome.error.code, 'NOT_FOUND');
    assert.equal(outcome.result, undefined);
  });
});
