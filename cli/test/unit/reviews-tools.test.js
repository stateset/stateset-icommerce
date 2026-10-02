/**
 * Review Tools Test Suite
 *
 * Tests for cli/src/tools/reviews.js
 * Covers: create_review, get_review, list_reviews, approve_review,
 *         reject_review, get_review_summary, flag_review
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { reviewTools } from '../../src/tools/reviews.js';

// ============================================================================
// Helper: find tool by name
// ============================================================================

function findTool(name) {
  const tool = reviewTools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

// ============================================================================
// Mock factory
// ============================================================================

const mockReview = {
  id: 'rev_001',
  productId: 'prod_001',
  customerId: 'cust_001',
  rating: 4,
  title: 'Great product',
  body: 'Really enjoyed using this widget. Highly recommend.',
  status: 'approved',
  verifiedPurchase: true,
  helpfulCount: 2,
  reportedCount: 0,
  createdAt: '2026-01-10T00:00:00Z',
  updatedAt: '2026-01-11T00:00:00Z',
};

// Shape of the binding's ReviewSummaryOutput (index 0 = 1-star count).
const mockSummary = {
  productId: 'prod_001',
  totalReviews: 42,
  averageRating: 4.2,
  ratingDistribution: [2, 3, 5, 15, 17],
};

function makeReviewCommerce(overrides = {}) {
  return {
    reviews: {
      create: async (data) => ({ ...mockReview, ...data }),
      get: async (_id) => mockReview,
      // Only methods the real Reviews binding class has.
      list: async (_filters) => [mockReview],
      update: async (id, input) => ({ ...mockReview, id, ...input }),
      getSummary: async (_productId) => mockSummary,
      markReported: async (_id) => undefined,
      ...overrides,
    },
  };
}

// ============================================================================
// Module exports
// ============================================================================

describe('reviewTools -- module exports', () => {
  it('exports an array of 7 tools', () => {
    assert.ok(Array.isArray(reviewTools));
    assert.equal(reviewTools.length, 7);
  });

  it('exports expected tool names in order', () => {
    const names = reviewTools.map((t) => t.name);
    assert.deepStrictEqual(names, [
      'create_review',
      'get_review',
      'list_reviews',
      'approve_review',
      'reject_review',
      'get_review_summary',
      'flag_review',
    ]);
  });

  it('all tools have handler functions', () => {
    for (const tool of reviewTools) {
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
    }
  });

  it('all tools have valid permissions', () => {
    for (const tool of reviewTools) {
      assert.ok(
        ['read', 'write', 'admin'].includes(tool.permission),
        `${tool.name} has invalid permission: ${tool.permission}`,
      );
    }
  });

  it('all tools have non-empty descriptions', () => {
    for (const tool of reviewTools) {
      assert.ok(tool.description, `${tool.name} missing description`);
      assert.ok(tool.description.length > 10, `${tool.name} description too short`);
    }
  });

  it('all tools have inputSchema objects', () => {
    for (const tool of reviewTools) {
      assert.equal(typeof tool.inputSchema, 'object', `${tool.name} missing inputSchema`);
    }
  });
});

// ============================================================================
// Input schema validation
// ============================================================================

describe('reviewTools -- input schemas', () => {
  it('create_review has productId, customerId, rating, title, body, orderId', () => {
    const schema = findTool('create_review').inputSchema;
    const keys = Object.keys(schema);
    assert.ok(keys.includes('productId'));
    assert.ok(keys.includes('customerId'));
    assert.ok(keys.includes('rating'));
    assert.ok(keys.includes('title'));
    assert.ok(keys.includes('body'));
    assert.ok(keys.includes('orderId'));
  });

  it('get_review has reviewId', () => {
    const schema = findTool('get_review').inputSchema;
    assert.ok(Object.keys(schema).includes('reviewId'));
  });

  it('list_reviews has productId, customerId, status, minRating, maxRating, limit', () => {
    const schema = findTool('list_reviews').inputSchema;
    const keys = Object.keys(schema);
    assert.ok(keys.includes('productId'));
    assert.ok(keys.includes('customerId'));
    assert.ok(keys.includes('status'));
    assert.ok(keys.includes('minRating'));
    assert.ok(keys.includes('maxRating'));
    assert.ok(keys.includes('limit'));
  });

  it('approve_review has reviewId', () => {
    const schema = findTool('approve_review').inputSchema;
    assert.ok(Object.keys(schema).includes('reviewId'));
  });

  it('reject_review has reviewId, reason', () => {
    const schema = findTool('reject_review').inputSchema;
    const keys = Object.keys(schema);
    assert.ok(keys.includes('reviewId'));
    assert.ok(keys.includes('reason'));
  });

  it('get_review_summary has productId', () => {
    const schema = findTool('get_review_summary').inputSchema;
    assert.ok(Object.keys(schema).includes('productId'));
  });

  it('flag_review has reviewId, reason, details', () => {
    const schema = findTool('flag_review').inputSchema;
    const keys = Object.keys(schema);
    assert.ok(keys.includes('reviewId'));
    assert.ok(keys.includes('reason'));
    assert.ok(keys.includes('details'));
  });
});

// ============================================================================
// Permission checks
// ============================================================================

describe('reviewTools -- permissions', () => {
  it('read tools have read permission', () => {
    assert.equal(findTool('get_review').permission, 'read');
    assert.equal(findTool('list_reviews').permission, 'read');
    assert.equal(findTool('get_review_summary').permission, 'read');
  });

  it('write tools have write permission', () => {
    assert.equal(findTool('create_review').permission, 'write');
    assert.equal(findTool('approve_review').permission, 'write');
    assert.equal(findTool('reject_review').permission, 'write');
    assert.equal(findTool('flag_review').permission, 'write');
  });
});

// ============================================================================
// Handler apply-guard (write tools without --apply)
// ============================================================================

describe('reviewTools -- apply-guard', () => {
  it('create_review requires --apply', async () => {
    const tool = findTool('create_review');
    const result = await tool.handler({
      params: { productId: 'prod_001', customerId: 'cust_001', rating: 5, body: 'Great!' },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('approve_review requires --apply', async () => {
    const tool = findTool('approve_review');
    const result = await tool.handler({
      params: { reviewId: 'rev_001' },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('reject_review requires --apply', async () => {
    const tool = findTool('reject_review');
    const result = await tool.handler({
      params: { reviewId: 'rev_001', reason: 'spam' },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('flag_review requires --apply', async () => {
    const tool = findTool('flag_review');
    const result = await tool.handler({
      params: { reviewId: 'rev_001', reason: 'spam' },
      allowApply: false,
      commerce: {},
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('apply-guard returns hint about --apply', async () => {
    const tool = findTool('create_review');
    const result = await tool.handler({
      params: { productId: 'prod_001', customerId: 'cust_001', rating: 5, body: 'Great!' },
      allowApply: false,
      commerce: {},
    });
    assert.ok(result.hint);
    assert.ok(result.hint.includes('--apply'));
  });

  it('apply-guard returns preview (wouldDo) with params', async () => {
    const params = { reviewId: 'rev_001', reason: 'inappropriate', details: 'Contains profanity' };
    const tool = findTool('flag_review');
    const result = await tool.handler({ params, allowApply: false, commerce: {} });
    assert.equal(result.success, false);
    assert.deepStrictEqual(result.wouldDo, params);
  });
});

// ============================================================================
// Handler success paths (with mocked commerce)
// ============================================================================

describe('reviewTools -- create_review handler', () => {
  it('creates review when allowApply is true', async () => {
    const tool = findTool('create_review');
    const result = await tool.handler({
      commerce: makeReviewCommerce(),
      params: { productId: 'prod_001', customerId: 'cust_001', rating: 5, body: 'Excellent!' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.message, 'Review created');
    assert.ok(result.review);
  });
});

describe('reviewTools -- get_review handler', () => {
  it('returns review with expected fields', async () => {
    const tool = findTool('get_review');
    const result = await tool.handler({
      commerce: makeReviewCommerce(),
      params: { reviewId: 'rev_001' },
    });
    assert.equal(result.success, true);
    assert.ok(result.review);
    assert.equal(result.review.id, 'rev_001');
    assert.equal(result.review.productId, 'prod_001');
    assert.equal(result.review.customerId, 'cust_001');
    assert.equal(result.review.rating, 4);
    assert.equal(result.review.title, 'Great product');
    assert.equal(result.review.status, 'approved');
    assert.equal(result.review.verifiedPurchase, true);
    assert.equal(result.review.reportedCount, 0);
    assert.equal(result.review.helpfulCount, 2);
    assert.ok(!('flagged' in result.review), 'flagged is not a ReviewOutput field');
    assert.ok(result.review.createdAt);
    assert.ok(result.review.updatedAt);
  });

  it('returns not found when review is null', async () => {
    const tool = findTool('get_review');
    const result = await tool.handler({
      commerce: makeReviewCommerce({ get: async () => null }),
      params: { reviewId: 'rev_missing' },
    });
    assert.equal(result.success, false);
    assert.equal(result.error, 'Review not found');
  });
});

describe('reviewTools -- list_reviews handler', () => {
  it('returns list with totalCount and returned', async () => {
    const tool = findTool('list_reviews');
    const result = await tool.handler({
      commerce: makeReviewCommerce(),
      params: { limit: 50 },
    });
    assert.equal(result.success, true);
    assert.equal(result.totalCount, 1);
    assert.equal(result.returned, 1);
    assert.ok(Array.isArray(result.reviews));
    assert.equal(result.reviews[0].id, 'rev_001');
  });

  it('maps expected fields on each review', async () => {
    const tool = findTool('list_reviews');
    const result = await tool.handler({
      commerce: makeReviewCommerce(),
      params: {},
    });
    const r = result.reviews[0];
    const expectedKeys = [
      'id',
      'productId',
      'customerId',
      'rating',
      'title',
      'status',
      'verifiedPurchase',
      'helpfulCount',
      'reportedCount',
      'createdAt',
    ];
    for (const key of expectedKeys) {
      assert.ok(key in r, `missing key: ${key}`);
    }
  });

  it('passes engine filters with an explicit page limit and applies maxRating locally', async () => {
    const tool = findTool('list_reviews');
    const calls = [];
    const reviews = [1, 2, 3, 4, 5].map((rating) => ({ ...mockReview, id: `r${rating}`, rating }));
    const result = await tool.handler({
      commerce: makeReviewCommerce({
        list: async (filter) => {
          calls.push(filter);
          // The engine applies minRating itself.
          return reviews.filter((r) => r.rating >= filter.minRating);
        },
      }),
      params: { productId: 'prod_001', status: 'approved', minRating: 2, maxRating: 4, limit: 2 },
    });
    assert.deepStrictEqual(calls, [
      {
        productId: 'prod_001',
        customerId: undefined,
        status: 'approved',
        minRating: 2,
        limit: 1000,
        offset: 0,
      },
    ]);
    assert.ok(!('maxRating' in calls[0]), 'ReviewFilterInput has no maxRating');
    assert.equal(result.totalCount, 3);
    assert.equal(result.returned, 2);
  });

  it('pages past the engine page cap so totalCount is exact', async () => {
    const tool = findTool('list_reviews');
    const offsets = [];
    const result = await tool.handler({
      commerce: makeReviewCommerce({
        list: async (filter) => {
          offsets.push(filter.offset);
          const n = filter.offset === 0 ? 1000 : 7;
          return Array.from({ length: n }, (_, i) => ({
            ...mockReview,
            id: `r${filter.offset + i}`,
          }));
        },
      }),
      params: { limit: 50 },
    });
    assert.deepStrictEqual(offsets, [0, 1000]);
    assert.equal(result.totalCount, 1007);
    assert.equal(result.returned, 50);
  });
});

describe('reviewTools -- approve_review handler', () => {
  it('approves review when allowApply is true', async () => {
    const tool = findTool('approve_review');
    let call;
    const result = await tool.handler({
      commerce: makeReviewCommerce({
        update: async (id, input) => {
          call = { id, input };
          return { ...mockReview, id, ...input };
        },
      }),
      params: { reviewId: 'rev_001' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.message, 'Review approved');
    assert.deepStrictEqual(call, { id: 'rev_001', input: { status: 'approved' } });
    assert.equal(result.review.status, 'approved');
  });
});

describe('reviewTools -- reject_review handler', () => {
  it('rejects review when allowApply is true', async () => {
    const tool = findTool('reject_review');
    let call;
    const result = await tool.handler({
      commerce: makeReviewCommerce({
        update: async (id, input) => {
          call = { id, input };
          return { ...mockReview, id, ...input };
        },
      }),
      params: { reviewId: 'rev_001', reason: 'Contains spam links' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.message, 'Review rejected');
    assert.deepStrictEqual(call, { id: 'rev_001', input: { status: 'rejected' } });
    assert.equal(result.review.status, 'rejected');
    // The engine has nowhere to store the reason; the tool must say so.
    assert.equal(result.reason, 'Contains spam links');
    assert.equal(result.reasonPersisted, false);
  });
});

describe('reviewTools -- get_review_summary handler', () => {
  it('returns review summary with expected fields', async () => {
    const tool = findTool('get_review_summary');
    const result = await tool.handler({
      commerce: makeReviewCommerce(),
      params: { productId: 'prod_001' },
    });
    assert.equal(result.success, true);
    assert.equal(result.productId, 'prod_001');
    assert.ok(result.summary);
    assert.equal(result.summary.totalReviews, 42);
    assert.equal(result.summary.averageRating, 4.2);
    assert.deepStrictEqual(result.summary.ratingDistribution, [2, 3, 5, 15, 17]);
    assert.deepStrictEqual(Object.keys(result.summary).sort(), [
      'averageRating',
      'ratingDistribution',
      'totalReviews',
    ]);
  });

  it('returns not found when the product has no approved reviews', async () => {
    const tool = findTool('get_review_summary');
    const result = await tool.handler({
      commerce: makeReviewCommerce({
        getSummary: async (productId) => ({
          productId,
          totalReviews: 0,
          averageRating: 0,
          ratingDistribution: [0, 0, 0, 0, 0],
        }),
      }),
      params: { productId: 'prod_missing' },
    });
    assert.equal(result.success, false);
    assert.equal(result.error, 'No approved reviews found for this product');
  });
});

describe('reviewTools -- flag_review handler', () => {
  it('flags review when allowApply is true', async () => {
    const tool = findTool('flag_review');
    const calls = [];
    const result = await tool.handler({
      commerce: makeReviewCommerce({
        markReported: async (id) => {
          calls.push(['markReported', id]);
        },
        update: async (id, input) => {
          calls.push(['update', id, input]);
          return { ...mockReview, id, ...input, reportedCount: 1 };
        },
      }),
      params: { reviewId: 'rev_001', reason: 'spam', details: 'Contains affiliate links' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.message, 'Review flagged for moderation');
    assert.deepStrictEqual(calls, [
      ['markReported', 'rev_001'],
      ['update', 'rev_001', { status: 'flagged' }],
    ]);
    assert.equal(result.review.status, 'flagged');
    assert.equal(result.review.reportedCount, 1);
    assert.equal(result.reason, 'spam');
    assert.equal(result.details, 'Contains affiliate links');
    assert.equal(result.reasonPersisted, false);
  });
});

// ============================================================================
// Handler error paths (commerce object missing methods)
// ============================================================================

describe('reviewTools -- error paths', () => {
  it('get_review throws when commerce.reviews is undefined', async () => {
    const tool = findTool('get_review');
    await assert.rejects(
      () => tool.handler({ commerce: {}, params: { reviewId: 'rev_001' } }),
      (err) => err instanceof TypeError,
    );
  });

  it('list_reviews throws when commerce.reviews is undefined', async () => {
    const tool = findTool('list_reviews');
    await assert.rejects(
      () => tool.handler({ commerce: {}, params: {} }),
      (err) => err instanceof TypeError,
    );
  });

  it('create_review throws when commerce.reviews.create is missing', async () => {
    const tool = findTool('create_review');
    await assert.rejects(
      () =>
        tool.handler({
          commerce: { reviews: {} },
          params: { productId: 'prod_001', customerId: 'cust_001', rating: 5, body: 'Great' },
          allowApply: true,
        }),
      (err) => err instanceof TypeError,
    );
  });

  it('approve_review throws when commerce.reviews.update is missing', async () => {
    const tool = findTool('approve_review');
    await assert.rejects(
      () =>
        tool.handler({
          commerce: { reviews: {} },
          params: { reviewId: 'rev_001' },
          allowApply: true,
        }),
      (err) => err instanceof TypeError,
    );
  });

  it('reject_review throws when commerce.reviews.update is missing', async () => {
    const tool = findTool('reject_review');
    await assert.rejects(
      () =>
        tool.handler({
          commerce: { reviews: {} },
          params: { reviewId: 'rev_001', reason: 'spam' },
          allowApply: true,
        }),
      (err) => err instanceof TypeError,
    );
  });

  it('get_review_summary throws when commerce.reviews.getSummary is missing', async () => {
    const tool = findTool('get_review_summary');
    await assert.rejects(
      () =>
        tool.handler({
          commerce: { reviews: {} },
          params: { productId: 'prod_001' },
        }),
      (err) => err instanceof TypeError,
    );
  });

  it('flag_review throws when commerce.reviews.markReported is missing', async () => {
    const tool = findTool('flag_review');
    await assert.rejects(
      () =>
        tool.handler({
          commerce: { reviews: {} },
          params: { reviewId: 'rev_001', reason: 'spam' },
          allowApply: true,
        }),
      (err) => err instanceof TypeError,
    );
  });
});
