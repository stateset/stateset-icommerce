/**
 * EDI Document Tools Module
 *
 * MCP tool definitions for EDI document exchange (850, 855, 856, 810, ...).
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const ediDocumentTools = withPolicyDomain('edi_documents', [
  {
    name: 'list_edi_documents',
    description: 'List EDI documents with optional filtering.',
    inputSchema: {
      documentType: z.string().min(1).optional().describe('Filter by document type, e.g. "850"'),
      direction: z.enum(['inbound', 'outbound']).optional().describe('Filter by direction'),
      status: z
        .enum(['pending', 'sent', 'acknowledged', 'processed', 'error'])
        .optional()
        .describe('Filter by status'),
      partner: z.string().min(1).optional().describe('Filter by trading partner'),
      limit: z.number().int().min(1).optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Offset for pagination'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const documents = await commerce.ediDocuments.list({
        documentType: params.documentType,
        direction: params.direction,
        status: params.status,
        partner: params.partner,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: documents.length, documents };
    },
  },
  {
    name: 'get_edi_document',
    description: 'Get an EDI document by ID.',
    inputSchema: {
      documentId: z.string().min(1).describe('EDI document ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const document = await commerce.ediDocuments.get(params.documentId);
      if (!document) {
        return { success: false, error: 'EDI document not found' };
      }
      return { success: true, document };
    },
  },
  {
    name: 'create_edi_document',
    description: 'Create / ingest an EDI document.',
    inputSchema: {
      documentType: z.string().min(1).describe('EDI document type, e.g. "850", "855", "856"'),
      direction: z
        .enum(['inbound', 'outbound'])
        .optional()
        .describe('Direction (defaults to inbound)'),
      partner: z.string().min(1).optional().describe('Trading partner name or ID'),
      reference: z
        .string()
        .min(1)
        .optional()
        .describe('Related business reference (PO number, order number, etc.)'),
      payload: z.string().optional().describe('Raw EDI payload'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create EDI document', params);
      }

      const document = await commerce.ediDocuments.create({
        documentType: params.documentType,
        direction: params.direction,
        partner: params.partner,
        reference: params.reference,
        payload: params.payload,
      });
      return { success: true, message: 'EDI document created', document };
    },
  },
  {
    name: 'set_edi_document_status',
    description: 'Update the status of an EDI document.',
    inputSchema: {
      documentId: z.string().min(1).describe('EDI document ID'),
      status: z
        .enum(['pending', 'sent', 'acknowledged', 'processed', 'error'])
        .describe('New status'),
      errorMessage: z
        .string()
        .max(2000)
        .optional()
        .describe('Failure detail when status is "error"'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Set EDI document status', params);
      }

      const document = await commerce.ediDocuments.setStatus(
        params.documentId,
        params.status,
        params.errorMessage,
      );
      return { success: true, message: 'EDI document status updated', document };
    },
  },
  {
    name: 'get_edi_summary',
    description: 'Get an aggregate summary of EDI documents (counts by status and type).',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const summary = await commerce.ediDocuments.summary();
      return { success: true, summary };
    },
  },
]);

export default ediDocumentTools;
