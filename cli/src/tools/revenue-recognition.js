/**
 * Revenue Recognition Tools Module
 *
 * MCP tool definitions for revenue contracts, schedules, and recognition.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

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
    description: 'Create a revenue recognition contract.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID'),
      totalValue: z.string().min(1).describe('Total contract value as an exact decimal string'),
      startDate: z.string().min(1).describe('Contract start date in ISO 8601'),
      endDate: z.string().min(1).optional().describe('Contract end date in ISO 8601'),
      currency: z.string().min(1).max(10).optional().describe('Optional currency code'),
      description: z.string().max(2000).optional().describe('Optional description'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create revenue contract', params);
      }

      const contract = await commerce.revenueRecognition.createContract({
        customerId: params.customerId,
        totalValue: params.totalValue,
        startDate: params.startDate,
        endDate: params.endDate,
        currency: params.currency,
        description: params.description,
      });
      return { success: true, message: 'Revenue contract created', contract };
    },
  },
  {
    name: 'generate_revenue_schedule',
    description: 'Generate the revenue recognition schedule for a contract.',
    inputSchema: {
      contractId: z.string().min(1).describe('Revenue contract ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Generate revenue schedule', params);
      }

      const schedule = await commerce.revenueRecognition.generateSchedule(params.contractId);
      return { success: true, message: 'Revenue schedule generated', schedule };
    },
  },
  {
    name: 'get_revenue_schedule',
    description: 'Get the revenue recognition schedule for a contract.',
    inputSchema: {
      contractId: z.string().min(1).describe('Revenue contract ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const schedule = await commerce.revenueRecognition.getSchedule(params.contractId);
      if (!schedule) {
        return { success: false, error: 'Revenue schedule not found' };
      }
      return { success: true, schedule };
    },
  },
  {
    name: 'recognize_revenue',
    description: 'Recognize revenue for a contract period.',
    inputSchema: {
      contractId: z.string().min(1).describe('Revenue contract ID'),
      periodDate: z.string().min(1).describe('Period date in ISO 8601'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Recognize revenue', params);
      }

      const result = await commerce.revenueRecognition.recognize(
        params.contractId,
        params.periodDate,
      );
      return { success: true, message: 'Revenue recognized', result };
    },
  },
]);

export default revenueRecognitionTools;
