import { editText, editorWindow } from './editor.js';
import { graphemes } from './text.js';
import { wrap } from './text.js';
import { terminalLayout } from './layout.js';
import { orderWorkflowKey, historyWorkflowLines, canReviewOrderAgain } from './order-workflow.js';
import { refundAction } from './refund-operations.js';

export function refundWorkflow(payment) {
  return {
    kind: 'refund-action',
    phase: 'edit',
    paymentId: payment.id,
    paymentLabel: payment.paymentNumber || payment.id,
    paymentCurrency: payment.currency,
    amount: '',
    reason: '',
    field: 0,
    cursor: 0,
    offset: 0,
    busy: false,
  };
}
export function refundWorkflowLimit(input, columns, rows) {
  const { size, available } = terminalLayout(columns, rows);
  return Math.max(0, refundWorkflowLines(input, size).length - Math.max(1, available - 1));
}
export function refundWorkflowKey(state, text, key) {
  const input = state.input;
  if (!input.busy && input.phase === 'edit') {
    if (key.name === 'escape') return { state: { ...state, input: null } };
    const field = input.field === 0 ? 'amount' : 'reason';
    if (key.name === 'tab' || (key.name === 'return' && input.field === 0)) {
      const next = input.field === 0 ? 'reason' : 'amount';
      return {
        state: {
          ...state,
          input: {
            ...input,
            field: input.field === 0 ? 1 : 0,
            cursor: graphemes(input[next]).length,
            error: null,
          },
        },
      };
    }
    if (key.name === 'return' || (key.ctrl && key.name === 's')) {
      try {
        const params = refundAction({
          paymentId: input.paymentId,
          amount: input.amount,
          reason: input.reason,
        });
        return {
          state: {
            ...state,
            input: { ...input, params, phase: 'previewing', busy: true, offset: 0, error: null },
          },
          effect: 'preview-refund',
        };
      } catch (error) {
        return { state: { ...state, input: { ...input, error: error.message } } };
      }
    }
    const edited = editText(
      input[field],
      input.cursor,
      text,
      key,
      field === 'amount'
        ? { limit: 60, accept: (value) => /^[0-9.]+$/.test(value) }
        : { limit: 240 },
    );
    return {
      state: edited
        ? {
            ...state,
            input: { ...input, [field]: edited.text, cursor: edited.cursor, error: null },
          }
        : state,
    };
  }
  return orderWorkflowKey(state, text, key, {
    name: 'refund',
    create: (current) =>
      refundWorkflow({
        id: current.paymentId,
        paymentNumber: current.paymentLabel,
        currency: current.paymentCurrency,
      }),
  });
}

export function refundWorkflowLines(input, size) {
  if (input.phase === 'history')
    return historyWorkflowLines(input, size, (entry) => ({
      resource: 'Payment',
      id: entry.paymentId,
      action: `${entry.currency} ${entry.amount} pending refund request`,
    }));
  const lines = [],
    add = (value = '') => lines.push(...wrap(value, size));
  if (input.phase === 'edit') {
    if (size >= 40) {
      add(`Payment: ${input.paymentLabel}`);
      add('Create one pending refund request. No provider transfer or settlement is performed.');
      add(
        `${input.field === 0 ? '›' : ' '} Amount (${input.paymentCurrency || 'payment currency'}): ${input.field === 0 ? editorWindow(input.amount, input.cursor, size - 16) : input.amount}`,
      );
      add(
        `${input.field === 1 ? '›' : ' '} Reason: ${input.field === 1 ? editorWindow(input.reason, input.cursor, size - 12) : input.reason}`,
      );
      add('Tab switches fields · Enter on reason or Ctrl+S previews · Esc cancels');
    } else {
      add(input.field === 0 ? `Amount ${input.paymentCurrency || ''}` : 'Refund reason');
      add(editorWindow(input.field === 0 ? input.amount : input.reason, input.cursor, size));
    }
  } else if (input.busy) {
    add(
      input.phase === 'applying'
        ? 'Creating the reviewed pending refund. Waiting for native receipt…'
        : input.phase === 'previewing'
          ? 'Previewing the refund against a private store snapshot…'
          : input.phase === 'history-loading'
            ? 'Loading local refund history. No command will run.'
            : 'Looking up the native refund receipt. No command will be repeated.',
    );
    add('Please wait. An active apply drains before shutdown.');
  } else if (input.phase === 'confirm') {
    add(`Create pending refund: ${input.preview.payment.currency} ${input.preview.params.amount}`);
    add(`Payment: ${input.preview.payment.paymentNumber || input.preview.payment.id}`);
    add(`Reason: ${input.preview.params.reason}`);
    add('No provider transfer or settlement is performed.');
    add(`Operator: ${input.preview.principalId} · store ${input.preview.storeId}`);
    add('Type APPLY to commit.');
    add(editorWindow(input.text, input.cursor, size));
    add('Enter commits · Esc returns to review');
  } else {
    if (input.reviewIssues?.length) {
      add('RECEIPT MISMATCH');
      add(
        'Inspect the payment, refund and native receipt. No command will be repeated from this result.',
      );
      input.reviewIssues.forEach((issue) => add(issue));
    }
    if (input.phase === 'uncertain') {
      add('OUTCOME UNKNOWN');
      add(
        'Enter retries the original key. After restart, Payments → P looks up the native receipt.',
      );
    }
    if (
      input.receipt?.status === 'succeeded' &&
      input.receipt.result?.status === 'pending' &&
      !input.reviewIssues?.length
    )
      add('PENDING REFUND CREATED');
    if (input.preview) {
      const { payment, params } = input.preview;
      add(`Payment: ${payment.paymentNumber || payment.id}`);
      add(`Pending refund request: ${payment.currency} ${params.amount}`);
      add(`Reason: ${params.reason}`);
      add(`Recorded payment: ${payment.currency} ${payment.amountExact}`);
      add(`Completed refunds: ${payment.currency} ${payment.amountRefundedExact}`);
      add(
        'The native engine also checks pending refunds and disputes when previewing and applying.',
      );
      add(`Operator: ${input.preview.principalId} · store ${input.preview.storeId}`);
    }
    const receipt = input.receipt || input.preview?.receipt;
    if (input.historyId) {
      add(
        input.receipt
          ? `Native receipt: ${input.receipt.status}`
          : input.phase === 'failed'
            ? 'Receipt lookup unavailable.'
            : 'No native receipt found.',
      );
      add(`History action: ${input.historyId}`);
      add('Esc returns to refund history.');
    }
    if (input.message) add(input.message);
    if (receipt) {
      const prior = !input.receipt && ['uncertain', 'failed'].includes(input.phase);
      add(`${prior ? 'Prior preview receipt' : 'Native receipt'}: ${receipt.status}`);
      if (receipt.status === 'succeeded') {
        add(`Native refund status: ${receipt.result?.status || 'Unavailable'}`);
        add(`Refund: ${receipt.result?.id || receipt.aggregate_id}`);
        add(`Amount: ${receipt.result?.currency} ${receipt.result?.amount}`);
        add('No provider transfer or completed refund is claimed.');
      }
      add(`Command: ${receipt.command_id}`);
      add(`Idempotency key: ${receipt.idempotency_key}`);
      if (receipt.error_code) add(`Refused: ${receipt.error_code} · ${receipt.error_message}`);
      add(`Audit link: ${receipt.audit_hash || 'Unavailable'}`);
      if (input.raw) for (const line of JSON.stringify(receipt, null, 2).split('\n')) add(line);
      add('v full receipt · ↑↓ Home End scroll · Esc closes');
    }
    if (input.phase === 'review')
      add(
        input.preview?.applyAllowed
          ? 'Enter continues to typed confirmation. No live change has been made.'
          : 'Preview only. No live change has been made. --apply is required to commit.',
      );
    if (canReviewOrderAgain(input))
      add('r starts a fresh refund review; a new preview and typed APPLY are required.');
    add(
      'Refund creation records a pending request. Provider execution and completion are separate operations.',
    );
  }
  if (input.error) add(input.error);
  return lines;
}
