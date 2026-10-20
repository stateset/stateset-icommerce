import { ORDER_ACTIONS } from './order-operations.js';
import { editText, editorWindow } from './editor.js';
import { wrap } from './text.js';
import { terminalLayout } from './layout.js';

export function orderWorkflowLimit(input, columns, rows) {
  const { size, available } = terminalLayout(columns, rows);
  return Math.max(0, orderWorkflowLines(input, size).length - Math.max(1, available - 1));
}

export function orderWorkflow(order) {
  return {
    kind: 'order-action',
    phase: 'choose',
    orderId: order.id,
    orderLabel: order.orderNumber || order.id,
    selected: 0,
    offset: 0,
    busy: false,
  };
}

export function canReviewOrderAgain(input) {
  return Boolean(
    input.orderId &&
    !input.busy &&
    !input.reviewIssues?.length &&
    ['result', 'failed'].includes(input.phase),
  );
}

export function orderWorkflowKey(state, text, key) {
  const input = state.input,
    name = key.name;
  if (input.busy) return { state };
  const next = { ...input };
  if (name === 'escape') {
    if (input.phase === 'confirm')
      return { state: { ...state, input: { ...input, phase: 'review', text: '', error: null } } };
    return { state: { ...state, input: null } };
  }
  if (text === 'r' && canReviewOrderAgain(input))
    return {
      state: {
        ...state,
        input: {
          ...orderWorkflow({ id: input.orderId, orderNumber: input.orderLabel }),
          selected: input.selected || 0,
        },
      },
    };
  if (input.phase === 'choose') {
    if (['up', 'down'].includes(name))
      next.selected =
        (input.selected + (name === 'up' ? -1 : 1) + ORDER_ACTIONS.length) % ORDER_ACTIONS.length;
    if (name === 'return')
      return {
        state: { ...state, input: { ...input, busy: true, phase: 'previewing' } },
        effect: 'preview-order',
      };
  } else if (input.phase === 'confirm') {
    if (name === 'return') {
      if (input.text !== 'APPLY')
        next.error = 'Type APPLY exactly, then Enter. Esc returns to review.';
      else
        return {
          state: { ...state, input: { ...input, phase: 'applying', busy: true, error: null } },
          effect: 'apply-order',
        };
    } else {
      const edited = editText(input.text, input.cursor, text, key, {
        limit: 5,
        accept: (value) => /^[A-Za-z]+$/.test(value),
      });
      if (edited) Object.assign(next, { text: edited.text, cursor: edited.cursor, error: null });
    }
  } else if (name === 'return' && input.phase === 'review') {
    if (input.preview?.applyAllowed && input.preview.receipt?.status === 'previewed')
      Object.assign(next, { phase: 'confirm', text: '', cursor: 0, offset: 0 });
    else
      next.error = 'Preview only. Live actions require --db --apply and an allowed native preview.';
  } else if (name === 'return' && input.phase === 'uncertain') {
    return {
      state: { ...state, input: { ...input, phase: 'applying', busy: true, error: null } },
      effect: 'apply-order',
    };
  } else if (text === 'v' && (input.receipt || input.preview?.receipt)) {
    next.raw = !input.raw;
    next.offset = 0;
  } else if (['up', 'down', 'pageup', 'pagedown', 'home', 'end'].includes(name)) {
    next.offset =
      name === 'home'
        ? 0
        : name === 'end'
          ? Number.MAX_SAFE_INTEGER
          : Math.max(
              0,
              input.offset +
                (['up', 'pageup'].includes(name) ? -1 : 1) *
                  (['pageup', 'pagedown'].includes(name) ? 8 : 1),
            );
  }
  return { state: { ...state, input: next } };
}

export function orderWorkflowLines(input, size) {
  const lines = [],
    add = (value = '') => lines.push(...wrap(value, size));
  if (input.phase === 'choose') {
    add(`Order: ${input.orderLabel}`);
    ORDER_ACTIONS.forEach((action, index) =>
      add(`${index === input.selected ? '›' : ' '} ${action.label}`),
    );
    add('↑↓ selects · Enter previews · Esc closes');
    add('One order. Native policy and state checks decide whether the action is allowed.');
  } else if (input.busy) {
    add(
      input.phase === 'applying'
        ? 'Applying reviewed command. Waiting for native receipt…'
        : input.phase === 'reconciling'
          ? 'Looking up the last native receipt. No command will be repeated.'
          : 'Previewing against a private store snapshot…',
    );
    add(
      'Please wait. No second action can start from this review. Ctrl+C exits; an active apply drains before shutdown.',
    );
  } else if (input.phase === 'confirm') {
    add(`Apply ${input.preview.params.status} to one order:`);
    add(input.preview.order.orderNumber || input.preview.order.id);
    add(`Store scope: ${input.preview.storeId ?? 'operator configured'}`);
    add('Type APPLY to commit.');
    add(editorWindow(input.text, input.cursor, size));
    add('Enter commits · Esc returns to review');
  } else {
    if (input.reviewIssues?.length) {
      add('RECEIPT MISMATCH');
      add('Inspect the order and native receipt. No command will be repeated from this result.');
      for (const issue of input.reviewIssues) add(issue);
    }
    if (input.phase === 'uncertain') {
      add('OUTCOME UNKNOWN');
      add('Enter retries the original key. After restart, P looks up the native receipt.');
    }
    const preview = input.preview;
    if (preview) {
      add(`Order: ${preview.order.orderNumber || preview.order.id}`);
      add(`ID: ${preview.order.id}`);
      add(`${preview.order.status} → ${preview.params.status} · one order`);
      add(`Reviewed version: ${preview.receipt?.version_before ?? 'Unavailable'}`);
      add(
        `Order total: ${typeof preview.order.totalAmountExact === 'string' && preview.order.currency ? `${preview.order.currency} ${preview.order.totalAmountExact}` : 'Unavailable'}`,
      );
      add(
        `Operator: ${preview.principalId ?? 'configured principal'} · store ${preview.storeId ?? 'configured store'}`,
      );
      add('This status action does not issue a refund or force cancellation of captured payments.');
    }
    if (input.message) add(input.message);
    if (input.phase === 'uncertain') {
      add(
        'Outcome unknown. Enter retries the same host-held idempotency key. After restart, P reconciles the native receipt.',
      );
    }
    const receipt = input.receipt || preview?.receipt;
    if (receipt) {
      const prior = !input.receipt && ['uncertain', 'failed'].includes(input.phase);
      add(`${prior ? 'Prior preview receipt' : 'Native receipt'}: ${receipt.status}`);
      add(`Command: ${receipt.command_id}`);
      add(`Idempotency key: ${receipt.idempotency_key}`);
      if (receipt.aggregate_id) add(`Order aggregate: ${receipt.aggregate_id}`);
      if (receipt.result?.status && receipt.status === 'succeeded')
        add(`Native order status: ${receipt.result.status}`);
      if (receipt.error_code) add(`Refused: ${receipt.error_code} · ${receipt.error_message}`);
      add(
        `Version ${receipt.version_before ?? 'Unavailable'} → ${receipt.version_after ?? 'Unavailable'}`,
      );
      add(`Audit link: ${receipt.audit_hash ?? 'Unavailable'}`);
      add(`Retry guidance: ${receipt.retry ?? 'Unavailable'}`);
      add('v toggles full receipt · ↑↓ Home End scroll · Esc closes');
      if (input.raw) for (const line of JSON.stringify(receipt, null, 2).split('\n')) add(line);
    }
    if (canReviewOrderAgain(input))
      add('r starts a fresh review of this order · a new preview and typed APPLY are required.');
    if (input.phase === 'review')
      add(
        preview?.applyAllowed
          ? 'Enter continues to typed confirmation. No live change has been made.'
          : 'Preview only. No live change has been made. --apply is required to commit.',
      );
  }
  if (input.error) add(input.error);
  return lines;
}
