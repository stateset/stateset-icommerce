// The tool result contract: one outcome shape for every MCP tool call.
//
// Agents act on what a tool returns, and tools return many shapes: plain
// objects, `{ success: false, error }`, kernel execution receipts whose
// `status` is `rejected`, `--apply` previews, thrown binding errors with a
// stable `code`, and gate refusals (policy, permission, payment). This module
// is the single place that reads all of them and answers the only questions an
// agent needs answered first:
//
//   { ok: boolean,            // did the call do (or would it do) what was asked?
//     preview: boolean,       // true when nothing was mutated (no --apply, dry run)
//     result?: unknown,       // the tool's own payload, untouched
//     error?: {               // present exactly when ok === false
//       code: string,         // machine-readable, stable
//       message: string,      // human sentence
//       retryable: boolean,   // true only for transient classes
//       hint?: string,
//     },
//     notice?: { code, message, hint? } }  // why a result is a preview
//
// The contract never rewrites the tool's payload: `result` is the value the
// tool returned, by reference. In particular it never converts money strings
// to numbers (or anything else) -- exact decimal strings stay strings.

/** Version of the outcome shape, carried in `_meta` of MCP responses. */
export const TOOL_RESULT_CONTRACT_VERSION = '1.0.0';

/** Codes the contract itself assigns. Structured codes from the engine pass through verbatim. */
export const ToolErrorCode = Object.freeze({
  /** A failure with no structured code and no recognisable shape. */
  TOOL_ERROR: 'TOOL_ERROR',
  /** The input failed the tool's schema. */
  INVALID_INPUT: 'INVALID_INPUT',
  /** No such tool (or no handler for it) on this server. */
  UNKNOWN_TOOL: 'UNKNOWN_TOOL',
  /** The thing the call names does not exist. */
  NOT_FOUND: 'NOT_FOUND',
  /** The permission gate refused the call. */
  PERMISSION_DENIED: 'PERMISSION_DENIED',
  /** The MCP policy engine refused the call. */
  POLICY_DENIED: 'POLICY_DENIED',
  /** A before_tool_call hook refused the call. */
  HOOK_BLOCKED: 'HOOK_BLOCKED',
  /** The tool is priced and no valid payment credential came with the call. */
  PAYMENT_REQUIRED: 'PAYMENT_REQUIRED',
  /** The treasury refused to charge for the call. */
  TREASURY_BLOCKED: 'TREASURY_BLOCKED',
  /** A kernel receipt said `rejected` without an `error_code`. */
  KERNEL_REJECTED: 'KERNEL_REJECTED',
  /** A kernel receipt said `failed` without an `error_code`. */
  KERNEL_FAILED: 'KERNEL_FAILED',
  /** The call ran out of time. Retryable. */
  TIMEOUT: 'TIMEOUT',
  /** A rate limit refused the call. Retryable. */
  RATE_LIMITED: 'RATE_LIMITED',
});

/** Notice codes explaining why an `ok` outcome is a preview. */
export const ToolNoticeCode = Object.freeze({
  /** A write that runs only with --apply: nothing was mutated. */
  APPLY_REQUIRED: 'APPLY_REQUIRED',
  /** A dry run: every gate passed, nothing executed. */
  DRY_RUN: 'DRY_RUN',
  /** The kernel previewed the command (guards passed, no mutation). */
  KERNEL_PREVIEW: 'KERNEL_PREVIEW',
});

/**
 * Symbol under which the step executor hands `executeTool` the tool's raw,
 * uncompacted result. A symbol so it never reaches JSON (replay logs, plan
 * results) -- those keep the compacted copy.
 */
export const RAW_TOOL_RESULT = Symbol.for('stateset.mcp.rawToolResult');

const isPlainObject = (value) =>
  value !== null && typeof value === 'object' && !Array.isArray(value);

const nonEmptyString = (value) => (typeof value === 'string' && value.trim() ? value : null);

/** Engine / binding codes are UPPER_SNAKE; napi statuses (`GenericFailure`) are not. */
const STRUCTURED_CODE = /^[A-Z][A-Z0-9_]*$/;
/** Kernel receipt codes are dotted lower snake (`commerce.checkout.conflict`). */
const KERNEL_CODE = /^[a-z][a-z0-9_]*(\.[a-z0-9_]+)+$/;

const isStructuredCode = (value) =>
  typeof value === 'string' && (STRUCTURED_CODE.test(value) || KERNEL_CODE.test(value));

const APPLY_REQUIRED_MESSAGE = /--apply\b/;

/**
 * Message shapes the CLI itself produces, used only when nothing structured
 * names the failure. Ordered; first match wins.
 */
const MESSAGE_CODES = [
  [/^Invalid parameters for tool\b|\bvalidation (?:failed|error)\b/i, ToolErrorCode.INVALID_INPUT],
  [/^Unknown tool\b|^No executable handler for tool\b/i, ToolErrorCode.UNKNOWN_TOOL],
  // Not "not permitted": engine business rules use it ("refund not permitted on …").
  [/\bpermission denied\b|\binsufficient permissions?\b/i, ToolErrorCode.PERMISSION_DENIED],
  [/\bblocked by policy\b|\bpolicy (?:denied|violation)\b/i, ToolErrorCode.POLICY_DENIED],
  [/\btimed out\b|\btimeout\b|\bdeadline exceeded\b/i, ToolErrorCode.TIMEOUT],
  [/\brate limit(?:ed)?\b|\btoo many requests\b/i, ToolErrorCode.RATE_LIMITED],
  [/\bnot found\b/i, ToolErrorCode.NOT_FOUND],
];

const codeFromMessage = (message) => {
  if (typeof message !== 'string') return null;
  for (const [pattern, code] of MESSAGE_CODES) {
    if (pattern.test(message)) return code;
  }
  return null;
};

const ALWAYS_RETRYABLE = new Set([
  ToolErrorCode.TIMEOUT,
  ToolErrorCode.RATE_LIMITED,
  'ETIMEDOUT',
  'ECONNRESET',
  'EAI_AGAIN',
]);

/**
 * Whether retrying the same call can succeed without changing it. Deliberately
 * conservative: only concurrency conflicts, timeouts, rate limits and a busy
 * store qualify. A duplicate-SKU `CONFLICT` is not transient; a version
 * conflict is.
 */
export function isRetryable(code, message = '', details = null) {
  if (ALWAYS_RETRYABLE.has(code)) return true;
  const text = typeof message === 'string' ? message : '';
  if (code === 'CONFLICT') {
    return (
      (isPlainObject(details) && details.expectedVersion !== undefined) ||
      /\bversion conflict\b|\boptimistic lock\b|\bconcurren/i.test(text)
    );
  }
  if (code === 'DATABASE') return /\b(?:database is locked|SQLITE_BUSY|busy)\b/i.test(text);
  return false;
}

/** Kernel `RetryDisposition` values that mean "the same command can succeed later". */
const RETRYABLE_DISPOSITIONS = new Set(['same_key', 'after_conflict', 'after_delay']);
const DISPOSITION_HINTS = {
  same_key: 'Retry with exactly the same idempotency key.',
  after_conflict: 'Reload the aggregate and retry after resolving the conflict.',
  after_delay: 'Retry later with exactly the same idempotency key.',
};

/** A kernel `ExecutionReceipt` (see crates/stateset-core/src/kernel.rs). */
export function isKernelReceipt(value) {
  return (
    isPlainObject(value) &&
    typeof value.status === 'string' &&
    ('receipt_id' in value || 'error_code' in value || 'command_type' in value)
  );
}

const buildError = ({ code, message, retryable, hint }) => {
  const error = {
    code: code || ToolErrorCode.TOOL_ERROR,
    message: nonEmptyString(message) || 'Tool reported a failure',
    retryable: retryable === true,
  };
  const hintText = nonEmptyString(hint);
  if (hintText) error.hint = hintText;
  return error;
};

const failure = (error, { result, preview = false } = {}) => {
  const outcome = { ok: false, preview: Boolean(preview), error: buildError(error) };
  if (result !== undefined) outcome.result = result;
  return outcome;
};

const success = (result, { preview = false, notice = null } = {}) => {
  const outcome = { ok: true, preview: Boolean(preview) };
  if (result !== undefined) outcome.result = result;
  if (notice) {
    outcome.notice = { code: notice.code, message: nonEmptyString(notice.message) || notice.code };
    const hint = nonEmptyString(notice.hint);
    if (hint) outcome.notice.hint = hint;
  }
  return outcome;
};

/** Judge a kernel receipt on its own terms. */
function fromKernelReceipt(receipt, result, fallbackMessage) {
  const status = receipt.status;
  const errorCode = nonEmptyString(receipt.error_code);
  if (status === 'rejected' || status === 'failed' || (errorCode && status !== 'succeeded')) {
    const disposition = typeof receipt.retry === 'string' ? receipt.retry : null;
    return failure(
      {
        code:
          errorCode ||
          (status === 'failed' ? ToolErrorCode.KERNEL_FAILED : ToolErrorCode.KERNEL_REJECTED),
        message:
          nonEmptyString(receipt.error_message) ||
          nonEmptyString(fallbackMessage) ||
          `Kernel command ${receipt.command_type || ''} ${status}`.replace(/\s+/g, ' ').trim(),
        retryable: disposition ? RETRYABLE_DISPOSITIONS.has(disposition) : false,
        hint: disposition ? DISPOSITION_HINTS[disposition] : null,
      },
      { result, preview: status === 'previewed' },
    );
  }
  if (status === 'previewed') {
    return success(result, {
      preview: true,
      notice: {
        code: ToolNoticeCode.KERNEL_PREVIEW,
        message: 'The kernel previewed this command; nothing was committed.',
        hint: 'Run with --apply to commit it.',
      },
    });
  }
  return success(result);
}

/** First structured code a failed result carries, if any. */
function structuredCodeOf(result) {
  // `result.code` is deliberately not read: on a refusal it is as likely to be
  // a coupon or promotion code as an error code.
  const candidates = [
    result.errorCode,
    result.error_code,
    result.reasonCode,
    isPlainObject(result.error) ? result.error.code : null,
  ];
  return candidates.find(isStructuredCode) ?? null;
}

const messageOf = (result) =>
  nonEmptyString(result.error) ||
  (isPlainObject(result.error) ? nonEmptyString(result.error.message) : null) ||
  nonEmptyString(result.message) ||
  nonEmptyString(result.reason) ||
  null;

/**
 * Normalize the value a tool handler returned.
 *
 * @param {unknown} result
 * @returns {{ ok: boolean, preview: boolean, result?: unknown, error?: object, notice?: object }}
 */
export function normalizeToolResult(result) {
  if (!isPlainObject(result)) return success(result);

  if (isKernelReceipt(result.receipt)) {
    return fromKernelReceipt(result.receipt, result, messageOf(result));
  }
  if (isKernelReceipt(result) && typeof result.receipt_id === 'string') {
    return fromKernelReceipt(result, result, null);
  }

  const failed = result.success === false || (result.success !== true && Boolean(result.error));
  if (!failed) {
    if (result.preview === true) {
      return success(result, {
        preview: true,
        notice: {
          code: ToolNoticeCode.APPLY_REQUIRED,
          message: messageOf(result) || 'Preview only; nothing was mutated.',
          hint: result.hint || 'Run with --apply to enable write operations.',
        },
      });
    }
    return success(result);
  }

  const message = messageOf(result);
  const code = structuredCodeOf(result);

  // A write refused only because --apply is off is a preview, not an error.
  if (!code && (result.preview === true || APPLY_REQUIRED_MESSAGE.test(message || ''))) {
    return success(result, {
      preview: true,
      notice: {
        code: ToolNoticeCode.APPLY_REQUIRED,
        message: message || 'Preview only; nothing was mutated.',
        hint: nonEmptyString(result.hint) || 'Run with --apply to enable write operations.',
      },
    });
  }

  const resolved = code || codeFromMessage(message) || ToolErrorCode.TOOL_ERROR;
  const details = isPlainObject(result.error) ? result.error.details : result.details;
  return failure(
    {
      code: resolved,
      message,
      retryable:
        typeof result.retryable === 'boolean'
          ? result.retryable
          : isRetryable(resolved, message, details),
      hint:
        nonEmptyString(result.hint) ||
        nonEmptyString(result.remediation) ||
        (isPlainObject(result.error) ? nonEmptyString(result.error.hint) : null),
    },
    { result },
  );
}

/** Parse the binding's `{"code":…,"message":…}` envelope if a message still carries it. */
function unwrapEnvelope(message) {
  if (typeof message !== 'string' || !message.startsWith('{"code"')) return null;
  try {
    const parsed = JSON.parse(message);
    return isPlainObject(parsed) && typeof parsed.code === 'string' ? parsed : null;
  } catch {
    return null;
  }
}

/**
 * Normalize a thrown error (or an `{ message, code, details }` record of one).
 *
 * @param {unknown} error
 */
export function normalizeToolError(error) {
  const raw = isPlainObject(error) || error instanceof Error ? error : { message: String(error) };
  const envelope = unwrapEnvelope(raw.message);
  const message = envelope?.message ?? raw.message;
  const details = envelope?.details ?? raw.details ?? null;
  let code = envelope?.code ?? (isStructuredCode(raw.code) ? raw.code : null);
  if (!code && (raw.name === 'ZodError' || Array.isArray(raw.issues))) {
    code = ToolErrorCode.INVALID_INPUT;
  }
  code = code || codeFromMessage(message) || ToolErrorCode.TOOL_ERROR;
  return failure({
    code,
    message,
    retryable: isRetryable(code, message, details),
    hint: raw.hint ?? null,
  });
}

/** `formatValidationIssues` output (`[{ path, message }]`) as one line. */
function describeValidation(validation) {
  if (typeof validation === 'string') return validation;
  if (Array.isArray(validation)) {
    return validation
      .map((issue) =>
        isPlainObject(issue)
          ? `${issue.path ? `${issue.path}: ` : ''}${issue.message ?? issue.code ?? ''}`
          : String(issue),
      )
      .join('; ');
  }
  return JSON.stringify(validation);
}

/** Gate statuses shared by the step executor and the MCP wrapper. */
const GATE_CODES = {
  blocked: ToolErrorCode.HOOK_BLOCKED,
  policy_block: ToolErrorCode.POLICY_DENIED,
  permission_block: ToolErrorCode.PERMISSION_DENIED,
  payment_required: ToolErrorCode.PAYMENT_REQUIRED,
  treasury_block: ToolErrorCode.TREASURY_BLOCKED,
};

const GATE_HINTS = {
  payment_required: 'Pay the challenge in result and retry with the payment credential.',
};

/**
 * Normalize an outcome record from `executeToolStepInPlan` (the record behind
 * `server.executeTool`), or a gate refusal from the MCP wrapper.
 *
 * @param {{ status: string, error?: string|null, errorCode?: string, errorDetails?: object,
 *           result?: unknown, notes?: object, permission?: object, charge?: object,
 *           remediation?: string }} outcome
 * @param {unknown} [rawResult] - the uncompacted tool result, when available
 */
export function normalizeStepOutcome(outcome, rawResult) {
  const status = outcome?.status;
  const result = rawResult !== undefined ? rawResult : (outcome?.result ?? undefined);
  const message = nonEmptyString(outcome?.error);

  switch (status) {
    case 'success':
    case 'rollback_success':
      return normalizeToolResult(result);

    case 'dry_run_success':
      return success(result, {
        preview: true,
        notice: {
          code: ToolNoticeCode.DRY_RUN,
          message: 'Dry run: every gate passed and nothing executed.',
        },
      });

    case 'preview':
    case 'dry_run_blocked': {
      if (status === 'dry_run_blocked' && !outcome?.permission?.preview) {
        return failure(
          {
            code: ToolErrorCode.TREASURY_BLOCKED,
            message: message || 'Treasury charge blocked',
          },
          { result: result ?? undefined, preview: true },
        );
      }
      // A governed preview can itself say the command would be refused.
      if (result !== undefined && result !== null) {
        const judged = normalizeToolResult(result);
        if (!judged.ok) return { ...judged, preview: true };
      }
      return success(result ?? (outcome?.wouldDo ? { wouldDo: outcome.wouldDo } : undefined), {
        preview: true,
        notice: {
          code: ToolNoticeCode.APPLY_REQUIRED,
          message:
            nonEmptyString(outcome?.permission?.reason) ||
            message ||
            'Preview only; nothing was mutated.',
          hint: 'Run with --apply to enable write operations.',
        },
      });
    }

    case 'invalid':
      return failure(
        {
          code: outcome?.notes?.validation
            ? ToolErrorCode.INVALID_INPUT
            : codeFromMessage(message) || ToolErrorCode.INVALID_INPUT,
          message,
          hint: outcome?.notes?.validation
            ? `Fix the input: ${describeValidation(outcome.notes.validation)}`
            : null,
        },
        { result: result ?? undefined },
      );

    case 'error':
    case 'rollback_failed': {
      if (result !== undefined && result !== null) {
        const judged = normalizeToolResult(result);
        if (!judged.ok) return judged;
      }
      const thrown = normalizeToolError({
        message: message || 'Tool execution failed',
        code: outcome?.errorCode,
        details: outcome?.errorDetails,
      });
      if (result !== undefined && result !== null) thrown.result = result;
      return thrown;
    }

    default: {
      if (GATE_CODES[status]) {
        return failure(
          {
            code: GATE_CODES[status],
            message: message || status,
            hint: nonEmptyString(outcome?.remediation) || GATE_HINTS[status] || null,
          },
          { result: result ?? undefined },
        );
      }
      if (message) return normalizeToolError({ message, code: outcome?.errorCode });
      return normalizeToolResult(result);
    }
  }
}

/**
 * Check that a value is a well-formed contract outcome. Returns the list of
 * problems (empty when valid) so tests can show exactly what is wrong.
 *
 * @param {unknown} value
 * @returns {string[]}
 */
export function validateToolResultContract(value) {
  const problems = [];
  if (!isPlainObject(value)) return ['outcome is not an object'];
  if (typeof value.ok !== 'boolean') problems.push('ok is not a boolean');
  if (typeof value.preview !== 'boolean') problems.push('preview is not a boolean');
  if (value.ok === false) {
    const { error } = value;
    if (!isPlainObject(error)) problems.push('ok is false but error is missing');
    else {
      if (!nonEmptyString(error.code)) problems.push('error.code is not a non-empty string');
      if (typeof error.message !== 'string') problems.push('error.message is not a string');
      if (typeof error.retryable !== 'boolean') problems.push('error.retryable is not a boolean');
      if (error.hint !== undefined && typeof error.hint !== 'string')
        problems.push('error.hint is not a string');
    }
  } else if (value.error !== undefined) {
    problems.push('ok is true but error is present');
  }
  if (value.notice !== undefined) {
    if (!isPlainObject(value.notice) || !nonEmptyString(value.notice.code))
      problems.push('notice.code is not a non-empty string');
    if (!value.preview) problems.push('notice is present but preview is false');
  }
  const allowed = new Set(['ok', 'preview', 'result', 'error', 'notice']);
  for (const key of Object.keys(value)) {
    if (!allowed.has(key)) problems.push(`unexpected key ${key}`);
  }
  return problems;
}

/**
 * Attach the contract to an MCP `CallToolResult`: `structuredContent` carries
 * the outcome, `isError` is set exactly when `ok` is false, and the text
 * content is left as the tool's own JSON so text-only clients see what they
 * always saw.
 *
 * @param {{ content?: unknown[] }} response
 * @param {ReturnType<typeof normalizeToolResult>} outcome
 */
export function attachContractToResponse(response, outcome) {
  const next = { ...response, structuredContent: outcome };
  if (outcome.ok) delete next.isError;
  else next.isError = true;
  return next;
}

/**
 * Recover the contract outcome from a `server.executeTool` return value, which
 * carries it flattened alongside its legacy fields (`ok`, `preview`,
 * `failure`, `notice`, `result`).
 *
 * @param {{ ok: boolean, preview: boolean, failure?: object|null, notice?: object, result?: unknown }} executed
 */
export function toToolResultContract(executed) {
  const outcome = { ok: executed?.ok === true, preview: executed?.preview === true };
  if (executed?.result !== undefined && executed?.result !== null) outcome.result = executed.result;
  if (!outcome.ok) outcome.error = executed?.failure ?? buildError({});
  if (executed?.notice) outcome.notice = executed.notice;
  return outcome;
}
