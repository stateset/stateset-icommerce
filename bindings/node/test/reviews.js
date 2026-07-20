/**
 * Product Reviews API tests for @stateset/embedded Node.js bindings.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

test('Reviews: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('reviews API exists and is supported', async () => {
    assert.ok(commerce.reviews, 'reviews API should exist');
    assert.equal(await commerce.reviews.isSupported(), true);
  });

  const product = await commerce.products.create({
    name: 'Reviewed Widget',
    description: 'A widget worth reviewing',
    variants: [{ sku: 'REV-001', name: 'Default', price: 19.99 }],
  });
  const customer = await commerce.customers.create({
    email: 'reviewer@example.com',
    firstName: 'Rev',
    lastName: 'Iewer',
  });

  let review;
  await t.test('create returns a review with the expected fields', async () => {
    review = await commerce.reviews.create({
      productId: product.id,
      customerId: customer.id,
      rating: 5,
      title: 'Excellent',
      body: 'Works great, would buy again.',
      verifiedPurchase: true,
    });
    assert.equal(review.productId, product.id);
    assert.equal(review.customerId, customer.id);
    assert.equal(review.rating, 5);
    assert.equal(review.title, 'Excellent');
    assert.equal(review.verifiedPurchase, true);
    assert.equal(review.helpfulCount, 0);
    assert.equal(review.reportedCount, 0);
    assert.equal(typeof review.status, 'string');
    assert.ok(review.id);
  });

  await t.test('create rejects an out-of-range rating', async () => {
    await assert.rejects(
      commerce.reviews.create({ productId: product.id, customerId: customer.id, rating: 999 }),
      /rating must be between 1 and 5/i,
    );
  });

  await t.test('get fetches the review by id', async () => {
    const found = await commerce.reviews.get(review.id);
    assert.ok(found);
    assert.equal(found.id, review.id);
  });

  await t.test('update changes the rating and moderation status', async () => {
    const updated = await commerce.reviews.update(review.id, {
      rating: 4,
      status: 'approved',
    });
    assert.equal(updated.rating, 4);
    assert.equal(updated.status, 'approved');
  });

  await t.test('markHelpful and markReported bump the counters', async () => {
    await commerce.reviews.markHelpful(review.id);
    await commerce.reviews.markReported(review.id);
    const after = await commerce.reviews.get(review.id);
    assert.equal(after.helpfulCount, 1);
    assert.equal(after.reportedCount, 1);
  });

  await t.test('getSummary aggregates ratings for the product', async () => {
    const summary = await commerce.reviews.getSummary(product.id);
    assert.equal(summary.productId, product.id);
    assert.equal(summary.ratingDistribution.length, 5);
    assert.ok(summary.totalReviews >= 1, `expected >=1 review, got ${summary.totalReviews}`);
    // The single review is now 4 stars -> index 3.
    assert.equal(summary.ratingDistribution[3], 1);
    assert.equal(summary.averageRating, 4);
  });

  await t.test('list finds the product\'s reviews', async () => {
    const list = await commerce.reviews.list({ productId: product.id });
    assert.ok(list.some((r) => r.id === review.id), 'listed reviews include the created one');
  });

  await t.test('delete removes the review', async () => {
    await commerce.reviews.delete(review.id);
    const gone = await commerce.reviews.get(review.id);
    assert.equal(gone, null);
  });
});
