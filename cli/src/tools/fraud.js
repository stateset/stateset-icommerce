/**
 * Fraud Detection Tools Module
 *
 * MCP tool definitions for fraud assessment, rule management, and order review.
 * Modularized from mcp-server.js for better maintainability.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

/**
 * Fraud tool definitions
 */
export const fraudTools = [
  {
    name: 'assess_order_fraud',
    description: 'Run fraud assessment on an order. Returns a risk score and matched signals.',
    inputSchema: {
      orderId: z.string().min(1).describe('Order ID to assess'),
      customerIp: z.string().max(45).optional().describe('Customer IP address'),
      deviceFingerprint: z.string().max(500).optional().describe('Device fingerprint hash'),
      billingAddress: z
        .object({
          country: z.string().min(2).max(3).describe('Billing country code'),
          region: z.string().max(100).optional().describe('Billing state/province'),
          postalCode: z.string().max(20).optional().describe('Billing postal code'),
        })
        .optional()
        .describe('Billing address for geo-mismatch detection'),
      shippingAddress: z
        .object({
          country: z.string().min(2).max(3).describe('Shipping country code'),
          region: z.string().max(100).optional().describe('Shipping state/province'),
          postalCode: z.string().max(20).optional().describe('Shipping postal code'),
        })
        .optional()
        .describe('Shipping address for geo-mismatch detection'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const assessment = await commerce.fraud.assessOrder({
        orderId: params.orderId,
        customerIp: params.customerIp,
        deviceFingerprint: params.deviceFingerprint,
        billingAddress: params.billingAddress,
        shippingAddress: params.shippingAddress,
      });

      return {
        success: true,
        assessment: {
          id: assessment.id,
          orderId: assessment.orderId,
          riskScore: assessment.riskScore,
          riskLevel: assessment.riskLevel,
          recommendation: assessment.recommendation,
          signals: assessment.signals,
          matchedRules: assessment.matchedRules,
          assessedAt: assessment.assessedAt,
        },
      };
    },
  },

  {
    name: 'get_fraud_assessment',
    description: 'Get a fraud assessment by ID.',
    inputSchema: {
      assessmentId: z.string().min(1).describe('Fraud assessment ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { assessmentId } = params;
      const assessment = await commerce.fraud.getAssessment(assessmentId);

      if (!assessment) {
        return { success: false, error: 'Fraud assessment not found' };
      }

      return {
        success: true,
        assessment: {
          id: assessment.id,
          orderId: assessment.orderId,
          riskScore: assessment.riskScore,
          riskLevel: assessment.riskLevel,
          recommendation: assessment.recommendation,
          signals: assessment.signals,
          matchedRules: assessment.matchedRules,
          reviewStatus: assessment.reviewStatus,
          reviewedBy: assessment.reviewedBy,
          assessedAt: assessment.assessedAt,
          reviewedAt: assessment.reviewedAt,
        },
      };
    },
  },

  {
    name: 'list_fraud_signals',
    description: 'List fraud signals for an order or across all recent orders.',
    inputSchema: {
      orderId: z.string().min(1).optional().describe('Filter by order ID'),
      riskLevel: z
        .enum(['low', 'medium', 'high', 'critical'])
        .optional()
        .describe('Filter by risk level'),
      limit: z
        .number()
        .int()
        .min(1)
        .max(500)
        .optional()
        .default(50)
        .describe('Maximum number of signals to return'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { orderId, riskLevel, limit } = params;
      const signals = await commerce.fraud.listSignals({ orderId, riskLevel });
      const limited = signals.slice(0, limit);

      return {
        success: true,
        returned: limited.length,
        signals: limited.map((s) => ({
          id: s.id,
          orderId: s.orderId,
          type: s.type,
          description: s.description,
          severity: s.severity,
          metadata: s.metadata,
          detectedAt: s.detectedAt,
        })),
      };
    },
  },

  {
    name: 'create_fraud_rule',
    description: 'Create a custom fraud detection rule.',
    inputSchema: {
      name: z.string().min(1).max(255).describe('Rule name'),
      description: z.string().max(1000).optional().describe('Rule description'),
      condition: z
        .object({
          field: z
            .string()
            .min(1)
            .describe('Field to evaluate (e.g., order_amount, shipping_country, email_domain)'),
          operator: z
            .enum(['eq', 'neq', 'gt', 'gte', 'lt', 'lte', 'in', 'not_in', 'matches'])
            .describe('Comparison operator'),
          value: z
            .union([z.string(), z.number(), z.boolean(), z.array(z.string())])
            .describe('Value to compare against'),
        })
        .describe('Rule condition'),
      action: z
        .enum(['flag', 'block', 'review', 'score_adjust'])
        .describe('Action to take when rule matches'),
      scoreAdjustment: z
        .number()
        .int()
        .min(-100)
        .max(100)
        .optional()
        .describe('Score adjustment points (for score_adjust action)'),
      priority: z
        .number()
        .int()
        .min(1)
        .max(100)
        .optional()
        .default(50)
        .describe('Rule priority (1=highest)'),
      enabled: z.boolean().optional().default(true).describe('Whether rule is active'),
    },
    permission: 'admin',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create fraud rule', params);
      }

      const rule = await commerce.fraud.createRule({
        name: params.name,
        description: params.description,
        condition: params.condition,
        action: params.action,
        scoreAdjustment: params.scoreAdjustment,
        priority: params.priority || 50,
        enabled: params.enabled !== false,
      });
      return { success: true, message: 'Fraud rule created', rule };
    },
  },

  {
    name: 'update_fraud_rule',
    description: 'Update a fraud detection rule.',
    inputSchema: {
      ruleId: z.string().min(1).describe('Fraud rule ID'),
      name: z.string().min(1).max(255).optional().describe('Updated rule name'),
      description: z.string().max(1000).optional().describe('Updated description'),
      condition: z
        .object({
          field: z.string().min(1).describe('Field to evaluate'),
          operator: z
            .enum(['eq', 'neq', 'gt', 'gte', 'lt', 'lte', 'in', 'not_in', 'matches'])
            .describe('Comparison operator'),
          value: z
            .union([z.string(), z.number(), z.boolean(), z.array(z.string())])
            .describe('Value to compare against'),
        })
        .optional()
        .describe('Updated condition'),
      action: z
        .enum(['flag', 'block', 'review', 'score_adjust'])
        .optional()
        .describe('Updated action'),
      scoreAdjustment: z
        .number()
        .int()
        .min(-100)
        .max(100)
        .optional()
        .describe('Updated score adjustment'),
      priority: z.number().int().min(1).max(100).optional().describe('Updated priority'),
      enabled: z.boolean().optional().describe('Enable or disable the rule'),
    },
    permission: 'admin',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Update fraud rule', params);
      }

      const rule = await commerce.fraud.updateRule(params.ruleId, {
        name: params.name,
        description: params.description,
        condition: params.condition,
        action: params.action,
        scoreAdjustment: params.scoreAdjustment,
        priority: params.priority,
        enabled: params.enabled,
      });
      return { success: true, message: 'Fraud rule updated', rule };
    },
  },

  {
    name: 'review_flagged_order',
    description: 'Review a flagged order and mark it as approved or rejected.',
    inputSchema: {
      assessmentId: z.string().min(1).describe('Fraud assessment ID'),
      decision: z.enum(['approve', 'reject', 'escalate']).describe('Review decision'),
      reason: z.string().min(1).max(500).describe('Reason for the decision'),
      reviewerNote: z.string().max(1000).optional().describe('Internal reviewer note'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Review flagged order', params);
      }

      const result = await commerce.fraud.reviewOrder({
        assessmentId: params.assessmentId,
        decision: params.decision,
        reason: params.reason,
        reviewerNote: params.reviewerNote,
      });
      return {
        success: true,
        message: `Order ${params.decision === 'approve' ? 'approved' : params.decision === 'reject' ? 'rejected' : 'escalated'}`,
        assessment: result,
      };
    },
  },
];

export default fraudTools;
