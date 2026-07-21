/**
 * Units of Measure Tools Module
 *
 * MCP tool definitions for unit classes, units of measure, and conversion rules.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const unitOfMeasureTools = withPolicyDomain('units-of-measure', [
  {
    name: 'list_unit_classes',
    description: 'List unit classes (e.g. weight, volume).',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const unitClasses = await commerce.unitsOfMeasure.listClasses();
      return { success: true, count: unitClasses.length, unitClasses };
    },
  },
  {
    name: 'create_unit_class',
    description: 'Create a unit class.',
    inputSchema: {
      name: z.string().min(1).describe('Unit class name'),
      description: z.string().min(1).optional().describe('Optional description'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create unit class', params);
      }
      const unitClass = await commerce.unitsOfMeasure.createClass({
        name: params.name,
        description: params.description,
      });
      return { success: true, message: 'Unit class created', unitClass };
    },
  },
  {
    name: 'delete_unit_class',
    description: 'Delete a unit class.',
    inputSchema: {
      id: z.string().min(1).describe('Unit class ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete unit class', params);
      }
      await commerce.unitsOfMeasure.deleteClass(params.id);
      return { success: true, message: 'Unit class deleted' };
    },
  },
  {
    name: 'list_units_of_measure',
    description: 'List units of measure, optionally scoped to a unit class.',
    inputSchema: {
      classId: z.string().min(1).optional().describe('Filter by unit class ID'),
      limit: z.number().int().positive().optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Results to skip'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const unitsOfMeasure = await commerce.unitsOfMeasure.listUoms({
        classId: params.classId,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: unitsOfMeasure.length, unitsOfMeasure };
    },
  },
  {
    name: 'create_unit_of_measure',
    description: 'Create a unit of measure within a unit class.',
    inputSchema: {
      unitClassId: z.string().min(1).describe('Unit class ID'),
      name: z.string().min(1).describe('Unit name'),
      abbreviation: z.string().min(1).describe('Unit abbreviation'),
      factor: z.string().min(1).describe('Conversion factor relative to the class base unit'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create unit of measure', params);
      }
      const unitOfMeasure = await commerce.unitsOfMeasure.createUom({
        unitClassId: params.unitClassId,
        name: params.name,
        abbreviation: params.abbreviation,
        factor: params.factor,
      });
      return { success: true, message: 'Unit of measure created', unitOfMeasure };
    },
  },
  {
    name: 'set_base_unit_of_measure',
    description: 'Mark a unit of measure as the base unit for its class.',
    inputSchema: {
      id: z.string().min(1).describe('Unit of measure ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Set base unit of measure', params);
      }
      const unitOfMeasure = await commerce.unitsOfMeasure.setBaseUom(params.id);
      return { success: true, message: 'Base unit of measure set', unitOfMeasure };
    },
  },
  {
    name: 'delete_unit_of_measure',
    description: 'Delete a unit of measure.',
    inputSchema: {
      id: z.string().min(1).describe('Unit of measure ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete unit of measure', params);
      }
      await commerce.unitsOfMeasure.deleteUom(params.id);
      return { success: true, message: 'Unit of measure deleted' };
    },
  },
  {
    name: 'list_unit_conversion_rules',
    description: 'List unit conversion rules.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const rules = await commerce.unitsOfMeasure.listRules();
      return { success: true, count: rules.length, rules };
    },
  },
  {
    name: 'create_unit_conversion_rule',
    description: 'Create a unit conversion rule (system-wide or SKU-specific).',
    inputSchema: {
      ruleType: z.enum(['SYSTEM', 'SKU']).describe('Rule type'),
      fromUomId: z.string().min(1).describe('Source unit of measure ID'),
      toUomId: z.string().min(1).describe('Target unit of measure ID'),
      factor: z.string().min(1).describe('Conversion factor'),
      productId: z.string().min(1).optional().describe('Product ID (required for SKU rules)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Create unit conversion rule', params);
      }
      const rule = await commerce.unitsOfMeasure.createRule({
        ruleType: params.ruleType,
        fromUomId: params.fromUomId,
        toUomId: params.toUomId,
        factor: params.factor,
        productId: params.productId,
      });
      return { success: true, message: 'Unit conversion rule created', rule };
    },
  },
  {
    name: 'delete_unit_conversion_rule',
    description: 'Delete a unit conversion rule.',
    inputSchema: {
      id: z.string().min(1).describe('Unit conversion rule ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Delete unit conversion rule', params);
      }
      await commerce.unitsOfMeasure.deleteRule(params.id);
      return { success: true, message: 'Unit conversion rule deleted' };
    },
  },
]);

export default unitOfMeasureTools;
