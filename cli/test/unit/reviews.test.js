/**
 * Review Tools Test Suite
 *
 * Tests for the reviewTools module (cli/src/tools/reviews.js):
 * - create_review (write)
 * - get_review (read)
 * - list_reviews (read)
 * - approve_review (write)
 * - reject_review (write)
 * - get_review_summary (read)
 * - flag_review (write)
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { reviewTools } from '../../src/tools/reviews.js';

// ============================================================================
// Helper: find tool by name from a tools array
// ============================================================================

function findTool(tools, name) {
  const tool = tools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// Mock data
// ============================================================================

const mockReview = {
  id: 'rev_001',
  productId: 'prod_001',
  customerId: 'cust_001',
  rating: 4,
  title: 'Great product',
  body: 'Really enjoyed this',
  status: 'pending',
  verifiedPurchase: true,
  flagged: false,
  helpfulCount: 5,
  reportedCount: 0,
  createdAt: '2026-02-01T00:00:00Z',
  updatedAt: '2026-02-01T00:00:00Z',
};

const mockSummary = {
  productId: 'prod_001',
  averageRating: 4.2,
  totalReviews: 50,
  ratingDistribution: [2, 3, 5, 20, 20],
  verifiedPurchaseCount: 40,
  recommendedPercentage: 92,
};

// ============================================================================
// Mock commerce factory
// ============================================================================

function makeReviewCommerce(overrides = {}) {
  return {
    reviews: {
      create: async (data) => ({ ...mockReview, ...data }),
      get: async (id) => (id === 'rev_001' ? mockReview : null),
      list: async () => [mockReview],
      count: async () => 1,
      approve: async (id) => ({ ...mockReview, id, status: 'approved' }),
      reject: async (id, reason) => ({
        ...mockReview,
        id,
        status: 'rejected',
        rejectionReason: reason,
      }),
      getSummary: async (productId) => (productId === 'prod_001' ? mockSummary : null),
      flag: async (id, flagData) => ({
        ...mockReview,
        id,
        flagged: true,
        flagReason: flagData.reason,
      }),
      ...overrides,
    },
  };
}

// ============================================================================
// Structural sanity check
// ============================================================================

describe('Review Tools — structure', () => {
  it('exports an array', () => {
    assert.ok(Array.isArray(reviewTools));
  });

  it('exports exactly 7 tools', () => {
    assert.equal(reviewTools.length, 7);
  });

  it('every tool has name, handler, and permission', () => {
    for (const tool of reviewTools) {
      assert.ok(tool.name, `missing name`);
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
      assert.ok(tool.permission, `${tool.name} missing permission`);
    }
  });

  it('write tool permissions are correct', () => {
    const writeTools = ['create_review', 'approve_review', 'reject_review', 'flag_review'];
    for (const name of writeTools) {
      const tool = findTool(reviewTools, name);
      assert.equal(tool.permission, 'write', `${name} should have write permission`);
    }
  });

  it('read tool permissions are correct', () => {
    const readTools = ['get_review', 'list_reviews', 'get_review_summary'];
    for (const name of readTools) {
      const tool = findTool(reviewTools, name);
      assert.equal(tool.permission, 'read', `${name} should have read permission`);
    }
  });
});

// ============================================================================
// create_review
// ============================================================================

describe('create_review', () => {
  const tool = findTool(reviewTools, 'create_review');

  it('returns preview (success: false) without --apply', async () => {
    const result = await tool.handler({
      commerce: makeReviewCommerce(),
      params: {
        productId: 'prod_001',
        customerId: 'cust_001',
        rating: 4,
        body: 'Really enjoyed this',
      },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error);
    assert.ok(result.hint);
  });

  it('creates review with --apply and returns success: true', async () => {
    const result = await tool.handler({
      commerce: makeReviewCommerce(),
      params: {
        productId: 'prod_001',
        customerId: 'cust_001',
        rating: 5,
        body: 'Excellent!',
        title: 'Top notch',
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('created'));
    assert.ok(result.review);
    assert.equal(result.review.rating, 5);
  });

  it('passes all fields to commerce.reviews.create', async () => {
    let calledWith;
    const commerce = makeReviewCommerce({
      create: async (data) => {
        calledWith = data;
        return { ...mockReview, ...data };
      },
    });
    await tool.handler({
      commerce,
      params: {
        productId: 'prod_001',
        customerId: 'cust_001',
        rating: 3,
        body: 'Average',
        title: 'OK',
        orderId: 'ord_001',
      },
      allowApply: true,
    });
    assert.equal(calledWith.productId, 'prod_001');
    assert.equal(calledWith.customerId, 'cust_001');
    assert.equal(calledWith.rating, 3);
    assert.equal(calledWith.body, 'Average');
    assert.equal(calledWith.orderId, 'ord_001');
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeReviewCommerce({
      create: async () => {
        throw new Error('DB error');
      },
    });
    await assert.rejects(
      () =>
        tool.handler({
          commerce,
          params: { productId: 'p', customerId: 'c', rating: 4, body: 'text' },
          allowApply: true,
        }),
      /DB error/,
    );
  });
});

// ============================================================================
// get_review
// ============================================================================

describe('get_review', () => {
  const tool = findTool(reviewTools, 'get_review');

  it('returns review for valid ID', async () => {
    const result = await tool.handler({
      commerce: makeReviewCommerce(),
      params: { reviewId: 'rev_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.review.id, 'rev_001');
    assert.equal(result.review.rating, 4);
    assert.equal(result.review.status, 'pending');
    assert.equal(result.review.verifiedPurchase, true);
  });

  it('returns success: false for unknown review ID', async () => {
    const result = await tool.handler({
      commerce: makeReviewCommerce(),
      params: { reviewId: 'rev_nope' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('not found'));
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeReviewCommerce({
      get: async () => {
        throw new Error('connection lost');
      },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params: { reviewId: 'rev_001' } }),
      /connection lost/,
    );
  });
});

// ============================================================================
// list_reviews
// ============================================================================

describe('list_reviews', () => {
  const tool = findTool(reviewTools, 'list_reviews');

  it('returns list with counts', async () => {
    const result = await tool.handler({
      commerce: makeReviewCommerce(),
      params: {},
    });
    assert.equal(result.success, true);
    assert.equal(result.totalCount, 1);
    assert.equal(result.returned, 1);
    assert.equal(result.reviews.length, 1);
    assert.equal(result.reviews[0].id, 'rev_001');
    assert.equal(result.reviews[0].rating, 4);
  });

  it('passes filters to commerce.reviews.list', async () => {
    let calledFilter;
    const commerce = makeReviewCommerce({
      list: async (filter) => {
        calledFilter = filter;
        return [];
      },
      count: async () => 0,
    });
    await tool.handler({
      commerce,
      params: { productId: 'prod_001', status: 'approved', minRating: 3 },
    });
    assert.equal(calledFilter.productId, 'prod_001');
    assert.equal(calledFilter.status, 'approved');
    assert.equal(calledFilter.minRating, 3);
  });

  it('slices results to limit', async () => {
    const manyReviews = Array.from({ length: 10 }, (_, i) => ({ ...mockReview, id: `rev_00${i}` }));
    const commerce = makeReviewCommerce({
      list: async () => manyReviews,
      count: async () => 10,
    });
    const result = await tool.handler({ commerce, params: { limit: 3 } });
    assert.equal(result.returned, 3);
    assert.equal(result.reviews.length, 3);
    assert.equal(result.totalCount, 10);
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeReviewCommerce({
      list: async () => {
        throw new Error('query failed');
      },
    });
    await assert.rejects(() => tool.handler({ commerce, params: {} }), /query failed/);
  });
});

// ============================================================================
// approve_review
// ============================================================================

describe('approve_review', () => {
  const tool = findTool(reviewTools, 'approve_review');

  it('returns preview (success: false) without --apply', async () => {
    const result = await tool.handler({
      commerce: makeReviewCommerce(),
      params: { reviewId: 'rev_001' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error);
  });

  it('approves review with --apply and returns success: true', async () => {
    const result = await tool.handler({
      commerce: makeReviewCommerce(),
      params: { reviewId: 'rev_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('approved'));
    assert.equal(result.review.status, 'approved');
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeReviewCommerce({
      approve: async () => {
        throw new Error('review not found');
      },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params: { reviewId: 'rev_nope' }, allowApply: true }),
      /review not found/,
    );
  });
});

// ============================================================================
// reject_review
// ============================================================================

describe('reject_review', () => {
  const tool = findTool(reviewTools, 'reject_review');

  it('returns preview (success: false) without --apply', async () => {
    const result = await tool.handler({
      commerce: makeReviewCommerce(),
      params: { reviewId: 'rev_001', reason: 'Contains spam' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error);
  });

  it('rejects review with reason when --apply is set', async () => {
    let calledId, calledReason;
    const commerce = makeReviewCommerce({
      reject: async (id, reason) => {
        calledId = id;
        calledReason = reason;
        return { ...mockReview, id, status: 'rejected' };
      },
    });
    const result = await tool.handler({
      commerce,
      params: { reviewId: 'rev_001', reason: 'Violates policy' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('rejected'));
    assert.equal(calledId, 'rev_001');
    assert.equal(calledReason, 'Violates policy');
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeReviewCommerce({
      reject: async () => {
        throw new Error('reject failed');
      },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params: { reviewId: 'r', reason: 'bad' }, allowApply: true }),
      /reject failed/,
    );
  });
});

// ============================================================================
// get_review_summary
// ============================================================================

describe('get_review_summary', () => {
  const tool = findTool(reviewTools, 'get_review_summary');

  it('returns summary for valid product', async () => {
    const result = await tool.handler({
      commerce: makeReviewCommerce(),
      params: { productId: 'prod_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.productId, 'prod_001');
    assert.equal(result.summary.averageRating, 4.2);
    assert.equal(result.summary.totalReviews, 50);
    assert.equal(result.summary.ratingDistribution.length, 5);
    assert.equal(result.summary.verifiedPurchaseCount, 40);
    assert.equal(result.summary.recommendedPercentage, 92);
  });

  it('returns success: false when no reviews exist for product', async () => {
    const result = await tool.handler({
      commerce: makeReviewCommerce(),
      params: { productId: 'prod_no_reviews' },
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('No reviews'));
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeReviewCommerce({
      getSummary: async () => {
        throw new Error('summary query failed');
      },
    });
    await assert.rejects(
      () => tool.handler({ commerce, params: { productId: 'prod_001' } }),
      /summary query failed/,
    );
  });
});

// ============================================================================
// flag_review
// ============================================================================

describe('flag_review', () => {
  const tool = findTool(reviewTools, 'flag_review');

  it('returns preview (success: false) without --apply', async () => {
    const result = await tool.handler({
      commerce: makeReviewCommerce(),
      params: { reviewId: 'rev_001', reason: 'spam' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error);
  });

  it('flags review with reason when --apply is set', async () => {
    let calledId, calledData;
    const commerce = makeReviewCommerce({
      flag: async (id, data) => {
        calledId = id;
        calledData = data;
        return { ...mockReview, id, flagged: true };
      },
    });
    const result = await tool.handler({
      commerce,
      params: { reviewId: 'rev_001', reason: 'fake', details: 'Suspicious account' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.message.includes('flagged'));
    assert.equal(calledId, 'rev_001');
    assert.equal(calledData.reason, 'fake');
    assert.equal(calledData.details, 'Suspicious account');
  });

  it('flags review without optional details', async () => {
    const result = await tool.handler({
      commerce: makeReviewCommerce(),
      params: { reviewId: 'rev_001', reason: 'inappropriate' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.review.flagged, true);
  });

  it('returns error when commerce throws', async () => {
    const commerce = makeReviewCommerce({
      flag: async () => {
        throw new Error('flag operation failed');
      },
    });
    await assert.rejects(
      () =>
        tool.handler({
          commerce,
          params: { reviewId: 'rev_001', reason: 'spam' },
          allowApply: true,
        }),
      /flag operation failed/,
    );
  });
});
