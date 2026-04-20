/**
 * Reviews Commands Module
 */

function parseJsonArg(value, label) {
  try {
    return JSON.parse(value);
  } catch (error) {
    throw new Error(`Invalid ${label} JSON: ${error.message}`);
  }
}

function parseOptionalRating(value, usage) {
  if (value === undefined) return undefined;
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed < 1 || parsed > 5) {
    throw new Error(usage);
  }
  return parsed;
}

function buildFilters(args, usage) {
  const [productId, customerId, status, minRatingRaw, maxRatingRaw] = args;
  return {
    productId: productId || undefined,
    customerId: customerId || undefined,
    status: status || undefined,
    minRating: parseOptionalRating(minRatingRaw, usage),
    maxRating: parseOptionalRating(maxRatingRaw, usage),
  };
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'list': {
      const [productId, customerId, status, minRatingRaw, maxRatingRaw, limitRaw] = args;
      const usage =
        'Usage: reviews list [productId] [customerId] [status] [minRating] [maxRating] [limit]';
      const filters = buildFilters(
        [productId, customerId, status, minRatingRaw, maxRatingRaw],
        usage,
      );
      const reviews = await commerce.reviews.list(filters);
      const limit = limitRaw ? Number.parseInt(limitRaw, 10) : undefined;
      const limited = Number.isInteger(limit) && limit > 0 ? reviews.slice(0, limit) : reviews;
      return formatReviewList(limited, { output, jsonOutput });
    }

    case 'get': {
      const reviewId = args[0];
      if (!reviewId) throw new Error('Usage: reviews get <reviewId>');
      const review = await commerce.reviews.get(reviewId);
      if (!review) throw new Error(`Review not found: ${reviewId}`);
      return formatReviewDetail(review, { jsonOutput });
    }

    case 'create': {
      const payloadJson = args[0];
      if (!payloadJson) {
        throw new Error('Usage: reviews create <payloadJson>');
      }
      const review = await commerce.reviews.create(parseJsonArg(payloadJson, 'payload'));
      return {
        review,
        formatted: `Created review ${review.id}`,
      };
    }

    case 'approve': {
      const reviewId = args[0];
      if (!reviewId) throw new Error('Usage: reviews approve <reviewId>');
      const review = await commerce.reviews.approve(reviewId);
      return {
        review,
        formatted: `Approved review ${review.id}`,
      };
    }

    case 'reject': {
      const [reviewId, ...reasonParts] = args;
      if (!reviewId || reasonParts.length === 0) {
        throw new Error('Usage: reviews reject <reviewId> <reason>');
      }
      const review = await commerce.reviews.reject(reviewId, reasonParts.join(' '));
      return {
        review,
        formatted: `Rejected review ${review.id}`,
      };
    }

    case 'summary': {
      const productId = args[0];
      if (!productId) throw new Error('Usage: reviews summary <productId>');
      const summary = await commerce.reviews.getSummary(productId);
      if (!summary) throw new Error(`No reviews found for product: ${productId}`);
      return formatSummary(productId, summary, { jsonOutput });
    }

    case 'flag': {
      const [reviewId, reason, ...detailParts] = args;
      if (!reviewId || !reason) {
        throw new Error('Usage: reviews flag <reviewId> <reason> [details]');
      }
      const review = await commerce.reviews.flag(reviewId, {
        reason,
        details: detailParts.join(' ') || undefined,
      });
      return {
        review,
        formatted: `Flagged review ${review.id} for moderation`,
      };
    }

    case 'count': {
      const usage =
        'Usage: reviews count [productId] [customerId] [status] [minRating] [maxRating]';
      const count = await commerce.reviews.count(buildFilters(args, usage));
      return { count, formatted: `Review count: ${count}` };
    }

    default:
      throw new Error(
        `Unknown action: reviews ${action}\n\n` +
          'Available actions:\n' +
          '  list [productId] [customerId] [status] [minRating] [maxRating] [limit]  List reviews\n' +
          '  get <reviewId>                                                           Get review\n' +
          '  create <payloadJson>                                                     Create review\n' +
          '  approve <reviewId>                                                       Approve review\n' +
          '  reject <reviewId> <reason>                                               Reject review\n' +
          '  summary <productId>                                                      Get review summary\n' +
          '  flag <reviewId> <reason> [details]                                       Flag review\n' +
          '  count [productId] [customerId] [status] [minRating] [maxRating]          Count reviews',
      );
  }
}

function formatReviewList(reviews, { output, jsonOutput }) {
  if (jsonOutput) return reviews;
  if (reviews.length === 0) return { formatted: 'No reviews found.' };
  const formatted = output.table(reviews, [
    { key: 'id', header: 'ID' },
    { key: 'productId', header: 'Product' },
    { key: 'customerId', header: 'Customer' },
    { key: 'rating', header: 'Rating', align: 'right' },
    { key: 'status', header: 'Status' },
    { key: 'flagged', header: 'Flagged' },
  ]);
  return { reviews, formatted };
}

function formatReviewDetail(review, { jsonOutput }) {
  if (jsonOutput) return review;
  return {
    review,
    formatted:
      `Review: ${review.id}\n` +
      `${'-'.repeat(32)}\n` +
      `Product:      ${review.productId}\n` +
      `Customer:     ${review.customerId}\n` +
      `Rating:       ${review.rating}\n` +
      `Status:       ${review.status}\n` +
      `Verified:     ${review.verifiedPurchase ? 'yes' : 'no'}\n` +
      `Flagged:      ${review.flagged ? 'yes' : 'no'}\n` +
      `Title:        ${review.title || 'N/A'}\n` +
      `Body:         ${review.body || 'N/A'}`,
  };
}

function formatSummary(productId, summary, { jsonOutput }) {
  if (jsonOutput) return { productId, ...summary };
  return {
    productId,
    summary,
    formatted:
      `Review summary for ${productId}\n` +
      `${'-'.repeat(36)}\n` +
      `Total reviews:    ${summary.totalReviews ?? 'N/A'}\n` +
      `Average rating:   ${summary.averageRating ?? 'N/A'}\n` +
      `Verified count:   ${summary.verifiedPurchaseCount ?? 'N/A'}\n` +
      `Recommended:      ${summary.recommendedPercentage ?? 'N/A'}`,
  };
}

export const metadata = {
  name: 'reviews',
  aliases: ['rev', 'review'],
  description: 'Product review and moderation commands',
  actions: {
    list: {
      description: 'List reviews',
      args: ['[productId]', '[customerId]', '[status]', '[minRating]', '[maxRating]', '[limit]'],
    },
    get: { description: 'Get review', args: ['<reviewId>'] },
    create: { description: 'Create review', args: ['<payloadJson>'] },
    approve: { description: 'Approve review', args: ['<reviewId>'] },
    reject: { description: 'Reject review', args: ['<reviewId>', '<reason>'] },
    summary: { description: 'Get review summary', args: ['<productId>'] },
    flag: { description: 'Flag review', args: ['<reviewId>', '<reason>', '[details]'] },
    count: {
      description: 'Count reviews',
      args: ['[productId]', '[customerId]', '[status]', '[minRating]', '[maxRating]'],
    },
  },
};

export default { execute, metadata };
