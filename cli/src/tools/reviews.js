/**
 * Review Tools Module
 *
 * MCP tool definitions for product review management, moderation, and summaries.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

/**
 * Review tool definitions
 */
export const reviewTools = [
  {
    name: 'create_review',
    description: 'Create a product review.',
    inputSchema: {
      productId: z.string().min(1).describe('Product ID'),
      customerId: z.string().min(1).describe('Customer ID'),
      rating: z.number().int().min(1).max(5).describe('Rating (1-5 stars)'),
      title: z.string().min(1).max(255).optional().describe('Review title'),
      body: z.string().min(1).max(5000).describe('Review body text'),
      orderId: z.string().min(1).optional().describe('Order ID (for verified purchase)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create review', params);
      }

      const review = await commerce.reviews.create({
        productId: params.productId,
        customerId: params.customerId,
        rating: params.rating,
        title: params.title,
        body: params.body,
        orderId: params.orderId,
      });
      return { success: true, message: 'Review created', review };
    },
  },

  {
    name: 'get_review',
    description: 'Get a review by ID.',
    inputSchema: {
      reviewId: z.string().min(1).describe('Review ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { reviewId } = params;
      const review = await commerce.reviews.get(reviewId);

      if (!review) {
        return { success: false, error: 'Review not found' };
      }

      return {
        success: true,
        review: {
          id: review.id,
          productId: review.productId,
          customerId: review.customerId,
          rating: review.rating,
          title: review.title,
          body: review.body,
          status: review.status,
          verifiedPurchase: review.verifiedPurchase,
          flagged: review.flagged,
          createdAt: review.createdAt,
          updatedAt: review.updatedAt,
        },
      };
    },
  },

  {
    name: 'list_reviews',
    description: 'List reviews with optional filters.',
    inputSchema: {
      productId: z.string().min(1).optional().describe('Filter by product ID'),
      customerId: z.string().min(1).optional().describe('Filter by customer ID'),
      status: z
        .enum(['pending', 'approved', 'rejected', 'flagged'])
        .optional()
        .describe('Filter by moderation status'),
      minRating: z.number().int().min(1).max(5).optional().describe('Minimum rating filter'),
      maxRating: z.number().int().min(1).max(5).optional().describe('Maximum rating filter'),
      limit: z
        .number()
        .int()
        .min(1)
        .max(500)
        .optional()
        .default(50)
        .describe('Maximum number of reviews to return'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { productId, customerId, status, minRating, maxRating, limit } = params;
      const reviews = await commerce.reviews.list({
        productId,
        customerId,
        status,
        minRating,
        maxRating,
      });
      const count = await commerce.reviews.count({
        productId,
        customerId,
        status,
        minRating,
        maxRating,
      });
      const limited = reviews.slice(0, limit);

      return {
        success: true,
        totalCount: count,
        returned: limited.length,
        reviews: limited.map((r) => ({
          id: r.id,
          productId: r.productId,
          customerId: r.customerId,
          rating: r.rating,
          title: r.title,
          status: r.status,
          verifiedPurchase: r.verifiedPurchase,
          flagged: r.flagged,
          createdAt: r.createdAt,
        })),
      };
    },
  },

  {
    name: 'approve_review',
    description: 'Approve a pending review for public display.',
    inputSchema: {
      reviewId: z.string().min(1).describe('Review ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Approve review', params);
      }

      const review = await commerce.reviews.approve(params.reviewId);
      return { success: true, message: 'Review approved', review };
    },
  },

  {
    name: 'reject_review',
    description: 'Reject a review with a reason.',
    inputSchema: {
      reviewId: z.string().min(1).describe('Review ID'),
      reason: z.string().min(1).max(500).describe('Reason for rejection'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Reject review', params);
      }

      const review = await commerce.reviews.reject(params.reviewId, params.reason);
      return { success: true, message: 'Review rejected', review };
    },
  },

  {
    name: 'get_review_summary',
    description:
      'Get aggregated review summary for a product including average rating and rating distribution.',
    inputSchema: {
      productId: z.string().min(1).describe('Product ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { productId } = params;
      const summary = await commerce.reviews.getSummary(productId);

      if (!summary) {
        return { success: false, error: 'No reviews found for this product' };
      }

      return {
        success: true,
        productId,
        summary: {
          totalReviews: summary.totalReviews,
          averageRating: summary.averageRating,
          ratingDistribution: summary.ratingDistribution,
          verifiedPurchaseCount: summary.verifiedPurchaseCount,
          recommendedPercentage: summary.recommendedPercentage,
        },
      };
    },
  },

  {
    name: 'flag_review',
    description: 'Flag a review for manual moderation.',
    inputSchema: {
      reviewId: z.string().min(1).describe('Review ID'),
      reason: z
        .enum(['spam', 'inappropriate', 'fake', 'off_topic', 'other'])
        .describe('Flag reason'),
      details: z.string().max(500).optional().describe('Additional details about the flag'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Flag review', params);
      }

      const review = await commerce.reviews.flag(params.reviewId, {
        reason: params.reason,
        details: params.details,
      });
      return { success: true, message: 'Review flagged for moderation', review };
    },
  },
];

export default reviewTools;
