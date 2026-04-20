/**
 * Wishlists Commands Module
 */

function parseJsonArg(value, label) {
  try {
    return JSON.parse(value);
  } catch (error) {
    throw new Error(`Invalid ${label} JSON: ${error.message}`);
  }
}

function parseBoolean(value) {
  return ['true', '1', 'yes', 'y'].includes(String(value || '').toLowerCase());
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'list': {
      const [customerId, limitRaw] = args;
      if (!customerId) throw new Error('Usage: wishlists list <customerId> [limit]');
      const wishlists = await commerce.wishlists.list({ customerId });
      const limit = limitRaw ? Number.parseInt(limitRaw, 10) : undefined;
      const limited = Number.isInteger(limit) && limit > 0 ? wishlists.slice(0, limit) : wishlists;
      return formatWishlistList(limited, { output, jsonOutput });
    }

    case 'get': {
      const wishlistId = args[0];
      if (!wishlistId) throw new Error('Usage: wishlists get <wishlistId>');
      const wishlist = await commerce.wishlists.get(wishlistId);
      if (!wishlist) throw new Error(`Wishlist not found: ${wishlistId}`);
      return formatWishlistDetail(wishlist, { output, jsonOutput });
    }

    case 'create': {
      const [customerId, name = 'My Wishlist', visibility = 'private'] = args;
      if (!customerId) {
        throw new Error('Usage: wishlists create <customerId> [name] [visibility]');
      }
      const wishlist = await commerce.wishlists.create({ customerId, name, visibility });
      return {
        wishlist,
        formatted: `Created wishlist ${wishlist.id}`,
      };
    }

    case 'add-item': {
      const [wishlistId, payloadJson] = args;
      if (!wishlistId || !payloadJson) {
        throw new Error('Usage: wishlists add-item <wishlistId> <payloadJson>');
      }
      const item = await commerce.wishlists.addItem(
        wishlistId,
        parseJsonArg(payloadJson, 'payload'),
      );
      return {
        item,
        formatted: `Added item ${item.id || item.productId} to wishlist ${wishlistId}`,
      };
    }

    case 'remove-item': {
      const [wishlistId, itemId] = args;
      if (!wishlistId || !itemId) {
        throw new Error('Usage: wishlists remove-item <wishlistId> <itemId>');
      }
      await commerce.wishlists.removeItem(wishlistId, itemId);
      return { formatted: `Removed item ${itemId} from wishlist ${wishlistId}` };
    }

    case 'convert': {
      const [wishlistId, clearWishlistRaw] = args;
      if (!wishlistId) throw new Error('Usage: wishlists convert <wishlistId> [clearWishlist]');
      const result = await commerce.wishlists.convertToCart(wishlistId, {
        clearWishlist: parseBoolean(clearWishlistRaw),
      });
      return formatConversion(wishlistId, result, { jsonOutput });
    }

    default:
      throw new Error(
        `Unknown action: wishlists ${action}\n\n` +
          'Available actions:\n' +
          '  list <customerId> [limit]             List wishlists\n' +
          '  get <wishlistId>                      Get wishlist\n' +
          '  create <customerId> [name] [visibility]  Create wishlist\n' +
          '  add-item <wishlistId> <payloadJson>   Add item to wishlist\n' +
          '  remove-item <wishlistId> <itemId>     Remove item from wishlist\n' +
          '  convert <wishlistId> [clearWishlist]  Convert wishlist to cart',
      );
  }
}

function formatWishlistList(wishlists, { output, jsonOutput }) {
  if (jsonOutput) return wishlists;
  if (wishlists.length === 0) return { formatted: 'No wishlists found.' };
  const formatted = output.table(wishlists, [
    { key: 'id', header: 'ID' },
    { key: 'name', header: 'Name' },
    { key: 'customerId', header: 'Customer' },
    { key: 'visibility', header: 'Visibility' },
    { key: 'itemCount', header: 'Items', align: 'right' },
  ]);
  return { wishlists, formatted };
}

function formatWishlistDetail(wishlist, { output, jsonOutput }) {
  if (jsonOutput) return wishlist;
  const items = Array.isArray(wishlist.items) ? wishlist.items : [];
  const itemsTable =
    items.length === 0
      ? 'No items'
      : output.table(items, [
          { key: 'id', header: 'Item' },
          { key: 'productId', header: 'Product' },
          { key: 'variantId', header: 'Variant' },
          { key: 'priority', header: 'Priority', align: 'right' },
        ]);
  return {
    wishlist,
    formatted:
      `Wishlist: ${wishlist.name}\n` +
      `${'-'.repeat(34)}\n` +
      `ID:           ${wishlist.id}\n` +
      `Customer:     ${wishlist.customerId}\n` +
      `Visibility:   ${wishlist.visibility}\n` +
      `Item count:   ${wishlist.itemCount ?? items.length}\n\n` +
      itemsTable,
  };
}

function formatConversion(wishlistId, result, { jsonOutput }) {
  if (jsonOutput) return result;
  return {
    wishlistId,
    result,
    formatted:
      `Converted wishlist ${wishlistId}\n` +
      `${'-'.repeat(34)}\n` +
      `Cart ID:            ${result.cartId}\n` +
      `Items added:        ${result.itemsAdded}\n` +
      `Items unavailable:  ${result.itemsUnavailable}`,
  };
}

export const metadata = {
  name: 'wishlists',
  aliases: ['wl', 'wishlist'],
  description: 'Wishlist creation and conversion commands',
  actions: {
    list: { description: 'List wishlists', args: ['<customerId>', '[limit]'] },
    get: { description: 'Get wishlist', args: ['<wishlistId>'] },
    create: { description: 'Create wishlist', args: ['<customerId>', '[name]', '[visibility]'] },
    'add-item': { description: 'Add item to wishlist', args: ['<wishlistId>', '<payloadJson>'] },
    'remove-item': { description: 'Remove item from wishlist', args: ['<wishlistId>', '<itemId>'] },
    convert: { description: 'Convert wishlist to cart', args: ['<wishlistId>', '[clearWishlist]'] },
  },
};

export default { execute, metadata };
