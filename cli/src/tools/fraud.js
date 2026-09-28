/**
 * Fraud Detection Tools Module
 *
 * MCP tool definitions for fraud assessment, rule management, and order review.
 *
 * The engine does not detect fraud signals itself: the caller supplies the
 * signals (type + 0.0-1.0 confidence score) and the engine records them,
 * derives the risk score (the highest signal score) and the initial decision.
 * Assessments are keyed by order ID -- there is no separate assessment ID.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

/** Every `FraudSignalType` the binding accepts. */
const FRAUD_SIGNAL_TYPES = [
  'velocity_spike',
  'address_mismatch',
  'high_value_first_order',
  'geo_ip_anomaly',
  'bin_country_mismatch',
  'device_fingerprint',
  'proxy_vpn',
  'disposable_email',
  'payment_retries',
  'unusual_time',
];

/** Every `FraudDecision` the binding accepts. */
const FRAUD_DECISIONS = ['accept', 'review', 'reject'];

const signalTypeSchema = z.enum(FRAUD_SIGNAL_TYPES);
const decisionSchema = z.enum(FRAUD_DECISIONS);
const scoreSchema = z.number().min(0).max(1);

/** Shape a `FraudAssessmentOutput` for a tool response. */
function formatAssessment(assessment) {
  return {
    orderId: assessment.orderId,
    riskScore: assessment.riskScore,
    decision: assessment.decision,
    needsReview: assessment.needsReview,
    signals: assessment.signals,
    reviewedBy: assessment.reviewedBy,
    reviewNotes: assessment.reviewNotes,
    createdAt: assessment.createdAt,
    updatedAt: assessment.updatedAt,
  };
}

/**
 * Fraud tool definitions
 */
export const fraudTools = [
  {
    name: 'assess_order_fraud',
    description:
      'Record a fraud assessment for an order from caller-supplied signals. The engine stores the ' +
      'signals, sets the risk score to the highest signal score, and decides accept (or review ' +
      'when the risk score is 0.8 or higher). One assessment per order.',
    inputSchema: {
      orderId: z.string().min(1).describe('Order ID to assess'),
      signals: z
        .array(
          z.object({
            signalType: signalTypeSchema.describe('Kind of fraud signal observed'),
            score: scoreSchema.describe('Signal confidence score (0.0 - 1.0)'),
            details: z.string().min(1).max(1000).describe('What was observed'),
          }),
        )
        .max(50)
        .describe('Fraud signals observed for the order (empty = no signals, risk score 0)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      // The assessment is persisted, so it goes through the apply gate even
      // though the tool is classified read.
      if (!allowApply) {
        return applyRequired('Record fraud assessment', params);
      }

      const assessment = await commerce.fraud.createAssessment({
        orderId: params.orderId,
        signals: params.signals.map((s) => ({
          signalType: s.signalType,
          score: s.score,
          details: s.details,
        })),
      });

      return { success: true, assessment: formatAssessment(assessment) };
    },
  },

  {
    name: 'get_fraud_assessment',
    description: 'Get the fraud assessment for an order.',
    inputSchema: {
      orderId: z.string().min(1).describe('Order ID whose assessment to fetch'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const assessment = await commerce.fraud.getAssessment(params.orderId);

      if (!assessment) {
        return { success: false, error: 'Fraud assessment not found' };
      }

      return { success: true, assessment: formatAssessment(assessment) };
    },
  },

  {
    name: 'list_fraud_signals',
    description:
      'List the fraud signals recorded on one order, or on the most recent assessments across orders.',
    inputSchema: {
      orderId: z.string().min(1).optional().describe('Only signals for this order'),
      minRiskScore: scoreSchema
        .optional()
        .describe('Only signals from assessments whose risk score is at least this (0.0 - 1.0)'),
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
      const { orderId, minRiskScore } = params;
      const limit = params.limit ?? 50;

      let assessments;
      if (orderId) {
        const assessment = await commerce.fraud.getAssessment(orderId);
        assessments = assessment ? [assessment] : [];
        if (minRiskScore !== undefined) {
          assessments = assessments.filter((a) => a.riskScore >= minRiskScore);
        }
      } else {
        // `limit` also bounds the assessment query; assessments that carry no
        // signals can make the result shorter than `limit`.
        assessments = await commerce.fraud.listAssessments({ minRiskScore, limit });
      }

      const signals = assessments.flatMap((a) => a.signals).slice(0, limit);

      return {
        success: true,
        returned: signals.length,
        signals: signals.map((s) => ({
          orderId: s.orderId,
          signalType: s.signalType,
          score: s.score,
          details: s.details,
          detectedAt: s.detectedAt,
        })),
      };
    },
  },

  {
    name: 'create_fraud_rule',
    description:
      'Create a fraud rule: when a signal of `signalType` scores at or above `threshold`, apply ' +
      '`action`. Rules are created enabled.',
    inputSchema: {
      name: z.string().min(1).max(255).describe('Rule name'),
      description: z.string().max(1000).optional().describe('Rule description'),
      signalType: signalTypeSchema.describe('Signal type the rule evaluates'),
      threshold: scoreSchema.describe('Signal score (0.0 - 1.0) at or above which the rule fires'),
      action: decisionSchema.describe('Decision to apply when the rule fires'),
    },
    permission: 'admin',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create fraud rule', params);
      }

      const rule = await commerce.fraud.createRule({
        name: params.name,
        description: params.description,
        signalType: params.signalType,
        threshold: params.threshold,
        action: params.action,
      });
      return { success: true, message: 'Fraud rule created', rule };
    },
  },

  {
    name: 'update_fraud_rule',
    description: 'Update a fraud rule (name, description, threshold, action, or enabled flag).',
    inputSchema: {
      ruleId: z.string().min(1).describe('Fraud rule ID'),
      name: z.string().min(1).max(255).optional().describe('Updated rule name'),
      description: z.string().max(1000).optional().describe('Updated description'),
      threshold: scoreSchema.optional().describe('Updated score threshold (0.0 - 1.0)'),
      action: decisionSchema.optional().describe('Updated decision'),
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
        threshold: params.threshold,
        action: params.action,
        enabled: params.enabled,
      });
      return { success: true, message: 'Fraud rule updated', rule };
    },
  },

  {
    name: 'review_flagged_order',
    description:
      "Record a manual review of an order's fraud assessment: set its decision, the reviewer, and why.",
    inputSchema: {
      orderId: z.string().min(1).describe('Order ID whose assessment is being reviewed'),
      decision: decisionSchema.describe('Review decision: accept, review (escalate), or reject'),
      reviewer: z.string().min(1).max(255).describe('Who made the decision'),
      reason: z
        .string()
        .min(1)
        .max(1000)
        .describe('Reason for the decision (stored as review notes)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Review flagged order', params);
      }

      const assessment = await commerce.fraud.reviewAssessment(
        params.orderId,
        params.decision,
        params.reviewer,
        params.reason,
      );
      const verb = { accept: 'accepted', reject: 'rejected', review: 'kept in review' }[
        params.decision
      ];
      return {
        success: true,
        message: `Order ${verb}`,
        assessment: formatAssessment(assessment),
      };
    },
  },
];

export default fraudTools;
