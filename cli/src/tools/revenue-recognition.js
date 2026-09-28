/**
 * Revenue Recognition Tools Module
 *
 * MCP tool definitions for revenue contracts, schedules, and recognition.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;
const DECIMAL = /^\d+(?:\.\d+)?$/;

const isoDate = () => z.string().regex(ISO_DATE, 'must be YYYY-MM-DD');
const decimal = () => z.string().regex(DECIMAL, 'must be an exact decimal string');

/**
 * Drop undefined/null keys: the binding's `Option<String>` fields accept an
 * absent key but refuse `null`.
 * @template {Record<string, unknown>} T
 * @param {T} input
 * @returns {T}
 */
function omitAbsent(input) {
  return /** @type {T} */ (
    Object.fromEntries(
      Object.entries(input).filter(([, value]) => value !== undefined && value !== null),
    )
  );
}

const obligationInput = z.object({
  description: z.string().min(1).describe('What is being delivered (e.g. "12-month SaaS access")'),
  allocatedAmount: decimal().describe(
    'Exact decimal amount allocated to this obligation; obligations must sum to transactionPrice',
  ),
  recognitionMethod: z
    .enum(['point_in_time', 'ratable_over_time', 'milestone'])
    .describe('How revenue for this obligation is recognized'),
  standaloneSellingPrice: decimal().optional().describe('Standalone selling price'),
  recognitionStart: isoDate().optional().describe('YYYY-MM-DD; required for ratable_over_time'),
  recognitionEnd: isoDate().optional().describe('YYYY-MM-DD; required for ratable_over_time'),
});

export const revenueRecognitionTools = withPolicyDomain('revenue_recognition', [
  {
    name: 'list_revenue_contracts',
    description: 'List revenue recognition contracts.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const contracts = await commerce.revenueRecognition.listContracts();
      return { success: true, count: contracts.length, contracts };
    },
  },
  {
    name: 'get_revenue_contract',
    description: 'Get a revenue recognition contract by ID.',
    inputSchema: {
      contractId: z.string().min(1).describe('Revenue contract ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const contract = await commerce.revenueRecognition.getContract(params.contractId);
      if (!contract) {
        return { success: false, error: 'Revenue contract not found' };
      }
      return { success: true, contract };
    },
  },
  {
    name: 'create_revenue_contract',
    description:
      'Create a revenue recognition (ASC 606) contract with its performance obligations. ' +
      "The obligations' allocated amounts must sum to the transaction price.",
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID'),
      transactionPrice: decimal().describe('Total transaction price as an exact decimal string'),
      effectiveDate: isoDate().describe('Contract effective date (YYYY-MM-DD)'),
      obligations: z
        .array(obligationInput)
        .min(1)
        .describe('Performance obligations (at least one)'),
      currency: z.string().length(3).optional().describe('Currency code, e.g. USD'),
      orderId: z.string().min(1).optional().describe('Originating order ID'),
      invoiceId: z.string().min(1).optional().describe('Originating invoice ID'),
      contractNumber: z
        .string()
        .min(1)
        .optional()
        .describe('Contract number (auto-generated RC-... when omitted)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create revenue contract', params);
      }

      // Binding CreateRevenueContractInput; absent optionals omitted, never null.
      const contract = await commerce.revenueRecognition.createContract(
        omitAbsent({
          contractNumber: params.contractNumber,
          customerId: params.customerId,
          orderId: params.orderId,
          invoiceId: params.invoiceId,
          transactionPrice: params.transactionPrice,
          currency: params.currency,
          effectiveDate: params.effectiveDate,
          obligations: params.obligations.map((obligation) => omitAbsent({ ...obligation })),
        }),
      );
      return { success: true, message: 'Revenue contract created', contract };
    },
  },
  {
    name: 'generate_revenue_schedule',
    description:
      "Generate the revenue recognition schedule for a performance obligation (ids are on the contract's obligations).",
    inputSchema: {
      obligationId: z.string().min(1).describe('Performance obligation ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Generate revenue schedule', params);
      }

      const schedule = await commerce.revenueRecognition.generateSchedule(params.obligationId);
      return { success: true, message: 'Revenue schedule generated', schedule };
    },
  },
  {
    name: 'get_revenue_schedule',
    description: 'Get the revenue recognition schedule for a performance obligation.',
    inputSchema: {
      obligationId: z.string().min(1).describe('Performance obligation ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const schedule = await commerce.revenueRecognition.getSchedule(params.obligationId);
      if (!schedule) {
        return { success: false, error: 'Revenue schedule not found' };
      }
      return { success: true, schedule };
    },
  },
  {
    name: 'recognize_revenue',
    description:
      'Recognize deferred revenue for a performance obligation: every scheduled entry whose period starts on or before `through`.',
    inputSchema: {
      obligationId: z.string().min(1).describe('Performance obligation ID'),
      through: isoDate().describe('Recognize entries with a period start on or before this date'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Recognize revenue', params);
      }

      const result = await commerce.revenueRecognition.recognize(
        params.obligationId,
        params.through,
      );
      return { success: true, message: 'Revenue recognized', result };
    },
  },
]);

export default revenueRecognitionTools;
