/**
 * Carts Commands Module
 */

function parseJsonArg(value, label) {
  try {
    return JSON.parse(value);
  } catch (error) {
    throw new Error(`Invalid ${label} JSON: ${error.message}`);
  }
}

function parseAmount(value, usage) {
  const amount = Number.parseFloat(value);
  if (!Number.isFinite(amount)) {
    throw new Error(usage);
  }
  return amount;
}

function parseQuantity(value, usage) {
  const quantity = Number.parseInt(value, 10);
  if (!Number.isInteger(quantity) || quantity <= 0) {
    throw new Error(usage);
  }
  return quantity;
}

function formatMoney(amount, currency) {
  if (amount === null || amount === undefined) return 'N/A';
  return currency ? `${currency} ${amount}` : String(amount);
}

async function getCartByIdentifier(commerce, identifier) {
  if (identifier.startsWith('CART-') && typeof commerce.carts.getByNumber === 'function') {
    return commerce.carts.getByNumber(identifier);
  }
  return commerce.carts.get(identifier);
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'list': {
      const [customerId, status] = args;
      const carts = customerId
        ? await commerce.carts.forCustomer(customerId)
        : await commerce.carts.list();
      const filtered = carts.filter((cart) => !status || cart.status === status);
      return formatCartList(filtered, { output, jsonOutput });
    }

    case 'get': {
      const identifier = args[0];
      if (!identifier) throw new Error('Usage: carts get <cartId|cartNumber>');
      const cart = await getCartByIdentifier(commerce, identifier);
      if (!cart) throw new Error(`Cart not found: ${identifier}`);
      return formatCartDetail(cart, { jsonOutput });
    }

    case 'create': {
      const [customerEmail, customerName, currency = 'USD', customerId] = args;
      const cart = await commerce.carts.create({
        customerEmail: customerEmail || undefined,
        customerName: customerName || undefined,
        currency: currency.toUpperCase(),
        customerId: customerId || undefined,
      });
      return {
        cart,
        formatted: `Created cart ${cart.cartNumber || cart.id}`,
      };
    }

    case 'items': {
      const cartId = args[0];
      if (!cartId) throw new Error('Usage: carts items <cartId>');
      const items = await commerce.carts.getItems(cartId);
      return formatCartItems(items, { output, jsonOutput });
    }

    case 'add': {
      const [cartId, sku, name, quantityRaw, unitPriceRaw, ...descriptionParts] = args;
      if (!cartId || !sku || !name || !quantityRaw || !unitPriceRaw) {
        throw new Error(
          'Usage: carts add <cartId> <sku> <name> <quantity> <unitPrice> [description]',
        );
      }
      const item = await commerce.carts.addItem(cartId, {
        sku,
        name,
        quantity: parseQuantity(
          quantityRaw,
          'Usage: carts add <cartId> <sku> <name> <quantity> <unitPrice> [description]',
        ),
        unitPrice: parseAmount(
          unitPriceRaw,
          'Usage: carts add <cartId> <sku> <name> <quantity> <unitPrice> [description]',
        ),
        description: descriptionParts.join(' ') || undefined,
      });
      return {
        item,
        formatted: `Added ${item.quantity} x ${item.sku} to cart ${cartId}`,
      };
    }

    case 'update-item': {
      const [itemId, quantityRaw] = args;
      if (!itemId || !quantityRaw) throw new Error('Usage: carts update-item <itemId> <quantity>');
      const item = await commerce.carts.updateItem(itemId, {
        quantity: parseQuantity(quantityRaw, 'Usage: carts update-item <itemId> <quantity>'),
      });
      return {
        item,
        formatted: `Updated cart item ${item.id} to quantity ${item.quantity}`,
      };
    }

    case 'remove-item': {
      const itemId = args[0];
      if (!itemId) throw new Error('Usage: carts remove-item <itemId>');
      await commerce.carts.removeItem(itemId);
      return { formatted: `Removed cart item ${itemId}` };
    }

    case 'clear': {
      const cartId = args[0];
      if (!cartId) throw new Error('Usage: carts clear <cartId>');
      await commerce.carts.clearItems(cartId);
      return { formatted: `Cleared items from cart ${cartId}` };
    }

    case 'shipping-address': {
      const [cartId, addressJson] = args;
      if (!cartId || !addressJson) {
        throw new Error('Usage: carts shipping-address <cartId> <addressJson>');
      }
      const cart = await commerce.carts.setShippingAddress(
        cartId,
        parseJsonArg(addressJson, 'address'),
      );
      return {
        cart,
        formatted: `Updated shipping address for cart ${cart.id}`,
      };
    }

    case 'billing-address': {
      const [cartId, addressJson] = args;
      if (!cartId || !addressJson) {
        throw new Error('Usage: carts billing-address <cartId> <addressJson>');
      }
      const cart = await commerce.carts.setBillingAddress(
        cartId,
        parseJsonArg(addressJson, 'address'),
      );
      return {
        cart,
        formatted: `Updated billing address for cart ${cart.id}`,
      };
    }

    case 'shipping': {
      const [cartId, addressJson, shippingMethod, shippingCarrier, shippingAmountRaw] = args;
      if (!cartId || !addressJson) {
        throw new Error(
          'Usage: carts shipping <cartId> <addressJson> [shippingMethod] [shippingCarrier] [shippingAmount]',
        );
      }
      const cart = await commerce.carts.setShipping(cartId, {
        shippingAddress: parseJsonArg(addressJson, 'address'),
        shippingMethod: shippingMethod || undefined,
        shippingCarrier: shippingCarrier || undefined,
        shippingAmount:
          shippingAmountRaw !== undefined
            ? parseAmount(
                shippingAmountRaw,
                'Usage: carts shipping <cartId> <addressJson> [shippingMethod] [shippingCarrier] [shippingAmount]',
              )
            : undefined,
      });
      return {
        cart,
        formatted: `Updated shipping selection for cart ${cart.id}`,
      };
    }

    case 'payment': {
      const [cartId, paymentMethod, paymentToken] = args;
      if (!cartId || !paymentMethod) {
        throw new Error('Usage: carts payment <cartId> <paymentMethod> [paymentToken]');
      }
      const cart = await commerce.carts.setPayment(cartId, { paymentMethod, paymentToken });
      return {
        cart,
        formatted: `Set payment method ${paymentMethod} for cart ${cart.id}`,
      };
    }

    case 'discount': {
      const [cartId, couponCode] = args;
      if (!cartId || !couponCode) throw new Error('Usage: carts discount <cartId> <couponCode>');
      const cart = await commerce.carts.applyDiscount(cartId, couponCode);
      return {
        cart,
        formatted: `Applied discount ${couponCode} to cart ${cart.id}`,
      };
    }

    case 'undiscount': {
      const cartId = args[0];
      if (!cartId) throw new Error('Usage: carts undiscount <cartId>');
      const cart = await commerce.carts.removeDiscount(cartId);
      return {
        cart,
        formatted: `Removed discount from cart ${cart.id}`,
      };
    }

    case 'rates': {
      const cartId = args[0];
      if (!cartId) throw new Error('Usage: carts rates <cartId>');
      const rates = await commerce.carts.getShippingRates(cartId);
      return formatShippingRates(rates, { output, jsonOutput });
    }

    case 'ready': {
      const cartId = args[0];
      if (!cartId) throw new Error('Usage: carts ready <cartId>');
      const cart = await commerce.carts.markReadyForPayment(cartId);
      return {
        cart,
        formatted: `Marked cart ${cart.id} ready for payment`,
      };
    }

    case 'begin': {
      const cartId = args[0];
      if (!cartId) throw new Error('Usage: carts begin <cartId>');
      const cart = await commerce.carts.beginCheckout(cartId);
      return {
        cart,
        formatted: `Started checkout for cart ${cart.id}`,
      };
    }

    case 'complete': {
      const cartId = args[0];
      if (!cartId) throw new Error('Usage: carts complete <cartId>');
      const result = await commerce.carts.complete(cartId);
      return {
        result,
        formatted: `Completed checkout for cart ${result.cartId}; created order ${result.orderNumber || result.orderId}`,
      };
    }

    case 'cancel': {
      const cartId = args[0];
      if (!cartId) throw new Error('Usage: carts cancel <cartId>');
      const cart = await commerce.carts.cancel(cartId);
      return {
        cart,
        formatted: `Cancelled cart ${cart.id}`,
      };
    }

    case 'abandon': {
      const cartId = args[0];
      if (!cartId) throw new Error('Usage: carts abandon <cartId>');
      const cart = await commerce.carts.abandon(cartId);
      return {
        cart,
        formatted: `Marked cart ${cart.id} as abandoned`,
      };
    }

    case 'expire': {
      const cartId = args[0];
      if (!cartId) throw new Error('Usage: carts expire <cartId>');
      const cart = await commerce.carts.expire(cartId);
      return {
        cart,
        formatted: `Expired cart ${cart.id}`,
      };
    }

    case 'reserve': {
      const cartId = args[0];
      if (!cartId) throw new Error('Usage: carts reserve <cartId>');
      const cart = await commerce.carts.reserveInventory(cartId);
      return {
        cart,
        formatted: `Reserved inventory for cart ${cart.id}`,
      };
    }

    case 'release': {
      const cartId = args[0];
      if (!cartId) throw new Error('Usage: carts release <cartId>');
      const cart = await commerce.carts.releaseInventory(cartId);
      return {
        cart,
        formatted: `Released reserved inventory for cart ${cart.id}`,
      };
    }

    case 'recalc': {
      const cartId = args[0];
      if (!cartId) throw new Error('Usage: carts recalc <cartId>');
      const cart = await commerce.carts.recalculate(cartId);
      return {
        cart,
        formatted: `Recalculated totals for cart ${cart.id}`,
      };
    }

    case 'tax': {
      const [cartId, taxAmountRaw] = args;
      if (!cartId || !taxAmountRaw) throw new Error('Usage: carts tax <cartId> <taxAmount>');
      const cart = await commerce.carts.setTax(
        cartId,
        parseAmount(taxAmountRaw, 'Usage: carts tax <cartId> <taxAmount>'),
      );
      return {
        cart,
        formatted: `Set tax for cart ${cart.id} to ${cart.taxAmount}`,
      };
    }

    case 'abandoned': {
      const carts = await commerce.carts.getAbandoned();
      return formatCartList(carts, { output, jsonOutput });
    }

    case 'expired': {
      const carts = await commerce.carts.getExpired();
      return formatCartList(carts, { output, jsonOutput });
    }

    default:
      throw new Error(
        `Unknown action: carts ${action}\n\n` +
          'Available actions:\n' +
          '  list [customerId] [status]                               List carts\n' +
          '  get <cartId|cartNumber>                                  Get cart details\n' +
          '  create [customerEmail] [customerName] [currency] [customerId] Create cart\n' +
          '  items <cartId>                                           List cart items\n' +
          '  add <cartId> <sku> <name> <quantity> <unitPrice> [description] Add item\n' +
          '  update-item <itemId> <quantity>                          Update item quantity\n' +
          '  remove-item <itemId>                                     Remove item\n' +
          '  clear <cartId>                                           Clear cart items\n' +
          '  shipping-address <cartId> <addressJson>                  Set shipping address\n' +
          '  billing-address <cartId> <addressJson>                   Set billing address\n' +
          '  shipping <cartId> <addressJson> [shippingMethod] [shippingCarrier] [shippingAmount]\n' +
          '  payment <cartId> <paymentMethod> [paymentToken]          Set payment method\n' +
          '  discount <cartId> <couponCode>                           Apply discount\n' +
          '  undiscount <cartId>                                      Remove discount\n' +
          '  rates <cartId>                                           Get shipping rates\n' +
          '  ready <cartId>                                           Mark ready for payment\n' +
          '  begin <cartId>                                           Begin checkout\n' +
          '  complete <cartId>                                        Complete checkout\n' +
          '  cancel <cartId>                                          Cancel cart\n' +
          '  abandon <cartId>                                         Mark cart abandoned\n' +
          '  expire <cartId>                                          Expire cart\n' +
          '  reserve <cartId>                                         Reserve inventory\n' +
          '  release <cartId>                                         Release inventory\n' +
          '  recalc <cartId>                                          Recalculate cart\n' +
          '  tax <cartId> <taxAmount>                                 Set cart tax\n' +
          '  abandoned                                                List abandoned carts\n' +
          '  expired                                                  List expired carts',
      );
  }
}

function formatCartList(carts, { output, jsonOutput }) {
  if (jsonOutput) return carts;
  if (carts.length === 0) return { formatted: 'No carts found.' };
  const formatted = output.table(
    carts.map((cart) => ({
      id: cart.id,
      cartNumber: cart.cartNumber,
      customerEmail: cart.customerEmail,
      status: cart.status,
      grandTotal: formatMoney(cart.grandTotal, cart.currency),
      itemCount: cart.itemCount,
    })),
    [
      { key: 'id', header: 'ID' },
      { key: 'cartNumber', header: 'Cart #' },
      { key: 'customerEmail', header: 'Customer' },
      { key: 'status', header: 'Status' },
      { key: 'grandTotal', header: 'Grand Total', align: 'right' },
      { key: 'itemCount', header: 'Items', align: 'right' },
    ],
  );
  return { carts, formatted };
}

function formatCartDetail(cart, { jsonOutput }) {
  if (jsonOutput) return cart;
  return {
    cart,
    formatted:
      `Cart: ${cart.cartNumber || cart.id}\n` +
      `${'-'.repeat(40)}\n` +
      `ID:            ${cart.id}\n` +
      `Customer:      ${cart.customerEmail || cart.customerId || 'N/A'}\n` +
      `Status:        ${cart.status}\n` +
      `Payment:       ${cart.paymentStatus || 'N/A'}\n` +
      `Subtotal:      ${formatMoney(cart.subtotal, cart.currency)}\n` +
      `Tax:           ${formatMoney(cart.taxAmount, cart.currency)}\n` +
      `Shipping:      ${formatMoney(cart.shippingAmount, cart.currency)}\n` +
      `Discount:      ${formatMoney(cart.discountAmount, cart.currency)}\n` +
      `Grand total:   ${formatMoney(cart.grandTotal, cart.currency)}\n` +
      `Items:         ${cart.itemCount ?? cart.items?.length ?? 0}`,
  };
}

function formatCartItems(items, { output, jsonOutput }) {
  if (jsonOutput) return items;
  if (items.length === 0) return { formatted: 'No cart items found.' };
  const formatted = output.table(items, [
    { key: 'id', header: 'ID' },
    { key: 'sku', header: 'SKU' },
    { key: 'name', header: 'Name' },
    { key: 'quantity', header: 'Qty', align: 'right' },
    { key: 'unitPrice', header: 'Unit', align: 'right' },
    { key: 'total', header: 'Total', align: 'right' },
  ]);
  return { items, formatted };
}

function formatShippingRates(rates, { output, jsonOutput }) {
  if (jsonOutput) return rates;
  if (rates.length === 0) return { formatted: 'No shipping rates found.' };
  const formatted = output.table(rates, [
    { key: 'id', header: 'ID' },
    { key: 'carrier', header: 'Carrier' },
    { key: 'service', header: 'Service' },
    { key: 'price', header: 'Price', align: 'right' },
    { key: 'currency', header: 'Currency' },
    { key: 'estimatedDays', header: 'ETA', align: 'right' },
  ]);
  return { rates, formatted };
}

export const metadata = {
  name: 'carts',
  aliases: ['cart', 'basket'],
  description: 'Cart lifecycle and checkout preparation commands',
  actions: {
    list: { description: 'List carts', args: ['[customerId]', '[status]'] },
    get: { description: 'Get cart', args: ['<cartId|cartNumber>'] },
    create: {
      description: 'Create cart',
      args: ['[customerEmail]', '[customerName]', '[currency]', '[customerId]'],
    },
    items: { description: 'List cart items', args: ['<cartId>'] },
    add: {
      description: 'Add item to cart',
      args: ['<cartId>', '<sku>', '<name>', '<quantity>', '<unitPrice>', '[description]'],
    },
    'update-item': { description: 'Update cart item quantity', args: ['<itemId>', '<quantity>'] },
    'remove-item': { description: 'Remove cart item', args: ['<itemId>'] },
    clear: { description: 'Clear cart items', args: ['<cartId>'] },
    'shipping-address': {
      description: 'Set shipping address',
      args: ['<cartId>', '<addressJson>'],
    },
    'billing-address': { description: 'Set billing address', args: ['<cartId>', '<addressJson>'] },
    shipping: {
      description: 'Set shipping selection',
      args: [
        '<cartId>',
        '<addressJson>',
        '[shippingMethod]',
        '[shippingCarrier]',
        '[shippingAmount]',
      ],
    },
    payment: {
      description: 'Set payment method',
      args: ['<cartId>', '<paymentMethod>', '[paymentToken]'],
    },
    discount: { description: 'Apply discount', args: ['<cartId>', '<couponCode>'] },
    undiscount: { description: 'Remove discount', args: ['<cartId>'] },
    rates: { description: 'Get shipping rates', args: ['<cartId>'] },
    ready: { description: 'Mark ready for payment', args: ['<cartId>'] },
    begin: { description: 'Begin checkout', args: ['<cartId>'] },
    complete: { description: 'Complete checkout', args: ['<cartId>'] },
    cancel: { description: 'Cancel cart', args: ['<cartId>'] },
    abandon: { description: 'Mark cart abandoned', args: ['<cartId>'] },
    expire: { description: 'Expire cart', args: ['<cartId>'] },
    reserve: { description: 'Reserve inventory', args: ['<cartId>'] },
    release: { description: 'Release reserved inventory', args: ['<cartId>'] },
    recalc: { description: 'Recalculate cart totals', args: ['<cartId>'] },
    tax: { description: 'Set cart tax', args: ['<cartId>', '<taxAmount>'] },
    abandoned: { description: 'List abandoned carts', args: [] },
    expired: { description: 'List expired carts', args: [] },
  },
};

export default { execute, metadata };
