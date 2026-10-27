import { zodToInputJsonSchema } from './mcp/zod-json-schema.js';
import { z } from 'zod';

export function createToolInputSchema(inputSchema = {}) {
  return z.object(inputSchema || {});
}

export function validateToolInput(inputSchema = {}, params = {}) {
  return createToolInputSchema(inputSchema).safeParse(params || {});
}

export function formatValidationIssues(error) {
  if (!error?.issues || !Array.isArray(error.issues)) return [];

  return error.issues.map((issue) => ({
    code: issue.code,
    message: issue.message,
    path: Array.isArray(issue.path) ? issue.path.join('.') : '',
  }));
}

export function inputSchemaDefToJsonSchema(inputSchema = {}) {
  return zodToInputJsonSchema(createToolInputSchema(inputSchema), { target: 'draft-2020-12' });
}
