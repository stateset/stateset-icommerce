/**
 * Custom Objects Tools Module
 *
 * MCP tool definitions for custom object (custom states / metaobjects) operations.
 */

import { z } from 'zod';

function normalizeValuesJson({ values, valuesJson }) {
  if (typeof valuesJson === 'string' && valuesJson.trim().length > 0) return valuesJson;
  if (values && typeof values === 'object') return JSON.stringify(values);
  return '{}';
}

/**
 * Custom object tool definitions
 */
export const customObjectTools = [
  // --------------------------------------------------------------------------
  // Types (schemas)
  // --------------------------------------------------------------------------
  {
    name: 'list_custom_object_types',
    description:
      'List custom object types (schemas). Custom objects are similar to Shopify metaobjects / Salesforce custom objects.',
    inputSchema: {
      search: z.string().optional().describe('Search by handle or display name'),
      limit: z.number().optional().describe('Max results'),
      offset: z.number().optional().describe('Offset'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const filter = {
        search: params.search,
        limit: params.limit,
        offset: params.offset,
      };
      const types = await commerce.customObjects.listTypes(filter);
      return { success: true, types };
    },
  },

  {
    name: 'get_custom_object_type',
    description: 'Get a custom object type (schema) by ID.',
    inputSchema: {
      id: z.string().describe('Custom object type ID (UUID)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const ty = await commerce.customObjects.getType(params.id);
      if (!ty) return { error: 'Custom object type not found' };
      return { success: true, type: ty };
    },
  },

  {
    name: 'get_custom_object_type_by_handle',
    description: 'Get a custom object type (schema) by handle.',
    inputSchema: {
      handle: z.string().describe('Custom object type handle (e.g., warranty_registration)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const ty = await commerce.customObjects.getTypeByHandle(params.handle);
      if (!ty) return { error: 'Custom object type not found' };
      return { success: true, type: ty };
    },
  },

  {
    name: 'create_custom_object_type',
    description:
      'Create a custom object type (schema). Fields define allowed keys and types; record values are validated deterministically.',
    inputSchema: {
      handle: z
        .string()
        .describe('Stable handle for the type (safe ASCII, e.g., warranty_registration)'),
      displayName: z.string().describe('Human display name'),
      description: z.string().optional().describe('Optional description'),
      fields: z
        .array(
          z.object({
            key: z.string().describe('Field key (safe ASCII, e.g., serial_number)'),
            fieldType: z
              .string()
              .describe('Field type: string|integer|decimal|boolean|date_time|uuid|json'),
            required: z.boolean().optional().default(false),
            list: z.boolean().optional().default(false),
            description: z.string().optional(),
          }),
        )
        .optional()
        .default([])
        .describe('Field definitions'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          error: 'Create operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldCreate: {
            handle: params.handle,
            displayName: params.displayName,
            fieldCount: params.fields?.length || 0,
          },
        };
      }

      const ty = await commerce.customObjects.createType({
        handle: params.handle,
        displayName: params.displayName,
        description: params.description,
        fields: params.fields || [],
      });
      return { success: true, message: 'Custom object type created', type: ty };
    },
  },

  {
    name: 'update_custom_object_type',
    description:
      'Update a custom object type (schema). Updating fields replaces the full field definition list.',
    inputSchema: {
      id: z.string().describe('Custom object type ID (UUID)'),
      displayName: z.string().optional().describe('New display name'),
      description: z.string().optional().describe('New description'),
      fields: z
        .array(
          z.object({
            key: z.string(),
            fieldType: z.string(),
            required: z.boolean().optional().default(false),
            list: z.boolean().optional().default(false),
            description: z.string().optional(),
          }),
        )
        .optional()
        .describe('Full replacement of field definitions'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          error: 'Update operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldUpdate: params,
        };
      }

      const updated = await commerce.customObjects.updateType(params.id, {
        displayName: params.displayName,
        description: params.description,
        fields: params.fields,
      });
      return { success: true, message: 'Custom object type updated', type: updated };
    },
  },

  {
    name: 'delete_custom_object_type',
    description:
      'Delete a custom object type (schema). Records of this type must be deleted first.',
    inputSchema: {
      id: z.string().describe('Custom object type ID (UUID)'),
    },
    permission: 'delete',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          error: 'Delete operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable destructive operations.',
          wouldDelete: { id: params.id },
        };
      }
      await commerce.customObjects.deleteType(params.id);
      return { success: true, message: 'Custom object type deleted' };
    },
  },

  // --------------------------------------------------------------------------
  // Records
  // --------------------------------------------------------------------------
  {
    name: 'list_custom_objects',
    description: 'List custom object records (entries).',
    inputSchema: {
      typeHandle: z.string().optional().describe('Filter by type handle'),
      ownerType: z.string().optional().describe('Filter by owner type'),
      ownerId: z.string().optional().describe('Filter by owner id'),
      handle: z.string().optional().describe('Filter by record handle'),
      limit: z.number().optional().describe('Max results'),
      offset: z.number().optional().describe('Offset'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const filter = {
        typeHandle: params.typeHandle,
        ownerType: params.ownerType,
        ownerId: params.ownerId,
        handle: params.handle,
        limit: params.limit,
        offset: params.offset,
      };
      const objects = await commerce.customObjects.listObjects(filter);
      return { success: true, objects };
    },
  },

  {
    name: 'get_custom_object',
    description: 'Get a custom object record by ID.',
    inputSchema: {
      id: z.string().describe('Custom object record ID (UUID)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const obj = await commerce.customObjects.getObject(params.id);
      if (!obj) return { error: 'Custom object not found' };
      return { success: true, object: obj };
    },
  },

  {
    name: 'get_custom_object_by_handle',
    description: 'Get a custom object record by (typeHandle, objectHandle).',
    inputSchema: {
      typeHandle: z.string().describe('Custom object type handle'),
      objectHandle: z.string().describe('Custom object record handle'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const obj = await commerce.customObjects.getObjectByHandle(
        params.typeHandle,
        params.objectHandle,
      );
      if (!obj) return { error: 'Custom object not found' };
      return { success: true, object: obj };
    },
  },

  {
    name: 'create_custom_object',
    description:
      'Create a custom object record. Provide `values` (object) or `valuesJson` (string). Values are validated against the type schema.',
    inputSchema: {
      typeHandle: z.string().describe('Custom object type handle'),
      handle: z.string().optional().describe('Optional record handle (unique within type)'),
      ownerType: z
        .string()
        .optional()
        .describe('Optional owner type (must be paired with ownerId)'),
      ownerId: z.string().optional().describe('Optional owner id (must be paired with ownerType)'),
      values: z.record(z.any()).optional().describe('Record values as an object'),
      valuesJson: z.string().optional().describe('Record values as a JSON string'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          error: 'Create operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldCreate: {
            typeHandle: params.typeHandle,
            handle: params.handle,
            ownerType: params.ownerType,
            ownerId: params.ownerId,
            valuesJson: normalizeValuesJson(params),
          },
        };
      }

      const obj = await commerce.customObjects.createObject({
        typeHandle: params.typeHandle,
        handle: params.handle,
        ownerType: params.ownerType,
        ownerId: params.ownerId,
        valuesJson: normalizeValuesJson(params),
      });
      return { success: true, message: 'Custom object created', object: obj };
    },
  },

  {
    name: 'update_custom_object',
    description:
      'Update a custom object record. Provide `values` (object) or `valuesJson` (string) to update record values.',
    inputSchema: {
      id: z.string().describe('Custom object record ID (UUID)'),
      handle: z.string().optional().describe('New handle'),
      ownerType: z.string().optional().describe('New owner type'),
      ownerId: z.string().optional().describe('New owner id'),
      values: z.record(z.any()).optional().describe('New values as an object'),
      valuesJson: z.string().optional().describe('New values as a JSON string'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          error: 'Update operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldUpdate: {
            ...params,
            valuesJson:
              params.valuesJson ?? (params.values ? JSON.stringify(params.values) : undefined),
          },
        };
      }

      const input = {
        handle: params.handle,
        ownerType: params.ownerType,
        ownerId: params.ownerId,
        valuesJson:
          typeof params.valuesJson === 'string'
            ? params.valuesJson
            : params.values
              ? JSON.stringify(params.values)
              : undefined,
      };

      const obj = await commerce.customObjects.updateObject(params.id, input);
      return { success: true, message: 'Custom object updated', object: obj };
    },
  },

  {
    name: 'delete_custom_object',
    description: 'Delete a custom object record by ID.',
    inputSchema: {
      id: z.string().describe('Custom object record ID (UUID)'),
    },
    permission: 'delete',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          error: 'Delete operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable destructive operations.',
          wouldDelete: { id: params.id },
        };
      }
      await commerce.customObjects.deleteObject(params.id);
      return { success: true, message: 'Custom object deleted' };
    },
  },
];

/**
 * Get all custom object tools
 */
export function getCustomObjectTools() {
  return customObjectTools;
}

/**
 * Get custom object tool by name
 */
export function getCustomObjectTool(name) {
  return customObjectTools.find((t) => t.name === name);
}

export default customObjectTools;
