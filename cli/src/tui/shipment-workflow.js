import { editText, editorWindow } from './editor.js';
import { graphemes, wrap } from './text.js';
import { terminalLayout } from './layout.js';
import { orderWorkflowKey, historyWorkflowLines, canReviewOrderAgain } from './order-workflow.js';
import { shipmentAction } from './shipment-operations.js';

export function shipmentWorkflow(order) {
  return {
    kind: 'shipment-action',
    phase: 'edit',
    orderId: order.id,
    orderLabel: order.orderNumber || order.id,
    lines: (order.items || []).map((item) => ({
      orderItemId: item.id,
      sku: item.sku,
      remaining: item.quantity - item.shippedQuantity,
      quantity: String(Math.max(0, item.quantity - item.shippedQuantity)),
    })),
    trackingNumber: '',
    field: 0,
    cursor: 0,
    offset: 0,
    busy: false,
  };
}
export function shipmentWorkflowLimit(input, columns, rows) {
  const { size, available } = terminalLayout(columns, rows);
  return Math.max(0, shipmentWorkflowLines(input, size).length - Math.max(1, available - 1));
}
export function shipmentWorkflowKey(state, text, key) {
  const input = state.input;
  if (text === 'r' && canReviewOrderAgain(input)) return { state };
  if (!input.busy && input.phase === 'edit') {
    if (key.name === 'escape') return { state: { ...state, input: null } };
    if (key.name === 'tab' || key.name === 'return') {
      if (key.name === 'tab' || input.field < input.lines.length) {
        const field = (input.field + 1) % (input.lines.length + 1);
        return {
          state: {
            ...state,
            input: {
              ...input,
              field,
              cursor: graphemes(field ? input.lines[field - 1].quantity : input.trackingNumber)
                .length,
              error: null,
            },
          },
        };
      }
    }
    if (key.name === 'return' || (key.ctrl && key.name === 's')) {
      try {
        if (
          input.lines.some(
            (line) =>
              !/^\d+$/.test(line.quantity) ||
              !Number.isSafeInteger(Number(line.quantity)) ||
              Number(line.quantity) > line.remaining,
          )
        )
          throw new Error('Each quantity must be a whole number from zero to its remaining units.');
        const params = shipmentAction({
          orderId: input.orderId,
          trackingNumber: input.trackingNumber,
          lines: input.lines
            .filter((line) => Number(line.quantity) > 0)
            .map((line) => ({ orderItemId: line.orderItemId, quantity: Number(line.quantity) })),
        });
        return {
          state: {
            ...state,
            input: { ...input, params, phase: 'previewing', busy: true, offset: 0, error: null },
          },
          effect: 'preview-shipment',
        };
      } catch (error) {
        return { state: { ...state, input: { ...input, error: error.message } } };
      }
    }
    const value = input.field ? input.lines[input.field - 1].quantity : input.trackingNumber;
    const edited = editText(
      value,
      input.cursor,
      text,
      key,
      input.field ? { limit: 10, accept: (v) => /^\d+$/.test(v) } : { limit: 200 },
    );
    if (!edited) return { state };
    return {
      state: {
        ...state,
        input: {
          ...input,
          cursor: edited.cursor,
          error: null,
          ...(input.field
            ? {
                lines: input.lines.map((line, index) =>
                  index === input.field - 1 ? { ...line, quantity: edited.text } : line,
                ),
              }
            : { trackingNumber: edited.text }),
        },
      },
    };
  }
  return orderWorkflowKey(state, text, key, {
    name: 'shipment',
    create: (current) =>
      shipmentWorkflow({
        id: current.orderId,
        orderNumber: current.orderLabel,
        items: current.preview?.order.items || [],
      }),
  });
}
export function shipmentWorkflowLines(input, size) {
  if (input.phase === 'history')
    return historyWorkflowLines(input, size, (entry) => ({
      resource: 'Order',
      id: entry.orderId,
      action: `Ship ${entry.quantity} units · ${entry.trackingNumber || 'no tracking'}`,
    }));
  const lines = [],
    add = (value = '') => lines.push(...wrap(value, size));
  if (input.phase === 'edit') {
    if (size >= 40) {
      add(`Order: ${input.orderLabel}`);
      add('Ship reserved physical stock. Zero skips a line; positive quantities ship now.');
      add(
        `${input.field === 0 ? '›' : ' '} Tracking: ${input.field === 0 ? editorWindow(input.trackingNumber, input.cursor, size - 14) : input.trackingNumber}`,
      );
      const start = Math.max(0, input.field - 1);
      input.lines
        .slice(start, start + 4)
        .forEach((line, index) =>
          add(
            `${input.field === start + index + 1 ? '›' : ' '} ${line.sku} · remaining ${line.remaining} · ship ${input.field === start + index + 1 ? editorWindow(line.quantity, input.cursor, 12) : line.quantity}`,
          ),
        );
      add(`Field ${input.field + 1}/${input.lines.length + 1} · Tab cycles through every line`);
      add('Tab/Enter next field · Ctrl+S previews · Esc cancels');
    } else {
      const line = input.lines[input.field - 1];
      add(input.field ? `${line.sku} / ${line.remaining}` : 'Tracking (optional)');
      add(editorWindow(input.field ? line.quantity : input.trackingNumber, input.cursor, size));
    }
  } else if (input.busy) {
    add(
      input.phase === 'applying'
        ? 'Shipping reviewed reserved stock. Waiting for native receipt…'
        : input.phase === 'previewing'
          ? 'Previewing shipment on a private store snapshot…'
          : 'Reading shipment history or native receipt. No shipment will be repeated.',
    );
    add('An active apply drains before shutdown.');
  } else if (input.phase === 'confirm') {
    add(`Ship reserved stock: ${input.preview.order.orderNumber || input.orderId}`);
    for (const line of input.preview.params.lines)
      add(
        `${input.preview.order.items.find((item) => item.id === line.orderItemId)?.sku || line.orderItemId}: ${line.quantity} units`,
      );
    add(`Tracking: ${input.preview.context.trackingNumber || 'None'}`);
    add(`Reviewed version: ${input.preview.order.version}`);
    add(`Operator: ${input.preview.principalId} · store ${input.preview.storeId}`);
    add('Type APPLY to commit.');
    add(editorWindow(input.text, input.cursor, size));
    add('Enter commits · Esc returns to review');
  } else {
    if (input.reviewIssues?.length) {
      add('RECEIPT MISMATCH');
      input.reviewIssues.forEach(add);
      add('Inspect stock and the native receipt before another dispatch.');
    }
    if (input.phase === 'uncertain') {
      add('OUTCOME UNKNOWN');
      add('Enter retries the original key. After restart, Orders → G looks up the native receipt.');
    }
    if (input.receipt?.status === 'succeeded' && !input.reviewIssues?.length)
      add('RESERVED STOCK SHIPPED');
    if (input.preview) {
      add(`Order: ${input.preview.order.orderNumber || input.orderId}`);
      for (const line of input.preview.params.lines)
        add(
          `${input.preview.order.items.find((item) => item.id === line.orderItemId)?.sku || line.orderItemId}: ship ${line.quantity} units`,
        );
      add(`Tracking: ${input.preview.context.trackingNumber || 'None'}`);
      add(
        `Order version: ${input.preview.order.version} · expected status ${input.preview.context.status}`,
      );
      add(`Operator: ${input.preview.principalId} · store ${input.preview.storeId}`);
    }
    if (input.message) add(input.message);
    const receipt = input.receipt || input.preview?.receipt;
    if (input.historyId) {
      add(`History action: ${input.historyId}`);
      add('Esc returns to shipment history.');
    }
    if (receipt) {
      add(
        `${!input.receipt && ['failed', 'uncertain'].includes(input.phase) ? 'Prior preview receipt' : 'Native receipt'}: ${receipt.status}`,
      );
      if (receipt.status === 'succeeded')
        add(`Native order status: ${receipt.result?.status || 'Unavailable'}`);
      add(`Command: ${receipt.command_id}`);
      add(`Idempotency key: ${receipt.idempotency_key}`);
      if (receipt.error_code) add(`Refused: ${receipt.error_code} · ${receipt.error_message}`);
      add(`Audit link: ${receipt.audit_hash || 'Unavailable'}`);
      if (input.raw) JSON.stringify(receipt, null, 2).split('\n').forEach(add);
      add('v full receipt · ↑↓ Home End scroll · Esc closes');
    }
    if (input.phase === 'review')
      add(
        input.preview?.applyAllowed
          ? 'Enter continues to typed confirmation. No live change has been made.'
          : 'Preview only. No live change has been made. --apply is required to commit.',
      );
    if (canReviewOrderAgain(input))
      add('Close and refresh the order before reviewing another dispatch.');
    add(
      'This native command consumes reservations and physical stock atomically. Carrier booking is separate.',
    );
  }
  if (input.error) add(input.error);
  return lines;
}
