export async function findPaymentByIdempotencyKey(commerce, idempotencyKey) {
  return (await commerce.payments.list()).find(
    (payment) => payment.idempotencyKey === idempotencyKey,
  );
}

export async function claimPaymentForOrder({ commerce, order, paymentInput }) {
  let payment;
  try {
    payment = await commerce.payments.createExact(paymentInput);
  } catch (error) {
    // A second process can lose the unique-key race after its initial read.
    // Re-read the winner instead of turning a safe replay into a 500.
    payment = await findPaymentByIdempotencyKey(commerce, paymentInput.idempotencyKey);
    if (!payment) throw error;
  }

  const amountMatches = payment.amountExact === paymentInput.amount;
  const currencyMatches =
    String(payment.currency).toUpperCase() === String(paymentInput.currency).toUpperCase();
  if (payment.orderId !== order.id || !amountMatches || !currencyMatches) {
    if (payment.orderId !== order.id && String(order.status).toLowerCase() === 'pending') {
      await commerce.orders.cancel(order.id);
    }
    return { conflict: true, payment };
  }
  return { conflict: false, payment };
}

export async function finishRecordedCheckout(commerce, payment, cartId) {
  if (!payment.orderId) throw new Error('Settlement claim has no order');
  const order = await commerce.orders.get(payment.orderId);
  if (!order) throw new Error('Settlement order was not found');
  if (!order.notes?.includes(`Cart: ${cartId}`)) {
    throw new Error('Settlement was already used for another checkout');
  }
  if (String(payment.status).toLowerCase() !== 'completed') {
    await commerce.payments.markCompleted(payment.id);
  }
  const confirmed =
    String(order.status).toLowerCase() === 'confirmed'
      ? order
      : await commerce.orders.updateStatus(order.id, 'confirmed');
  const cart = await commerce.carts.get(cartId);
  if (cart?.status === 'active') await commerce.carts.cancel(cartId);
  return confirmed;
}
