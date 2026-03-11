import { z } from 'zod';

function getSchemaDescription(schema) {
  return schema?.description || schema?._def?.description || undefined;
}

function withMetadata(jsonSchema, metadata) {
  const output = { ...(jsonSchema || {}) };

  if (metadata.description && !output.description) {
    output.description = metadata.description;
  }

  if (metadata.defaultValue !== undefined && output.default === undefined) {
    output.default = metadata.defaultValue;
  }

  if (metadata.nullable) {
    if (typeof output.type === 'string') {
      output.type = [output.type, 'null'];
    } else if (Array.isArray(output.type)) {
      output.type = Array.from(new Set([...output.type, 'null']));
    } else {
      output.anyOf = [...(output.anyOf || [output]), { type: 'null' }];
      delete output.type;
    }
  }

  return output;
}

function unwrapSchema(schema) {
  let current = schema;
  let optional = false;
  let nullable = false;
  let defaultValue;
  let description = getSchemaDescription(schema);

  while (current?._def?.typeName) {
    const typeName = current._def.typeName;
    if (typeName === 'ZodOptional') {
      optional = true;
      current = current._def.innerType;
      description ||= getSchemaDescription(current);
      continue;
    }
    if (typeName === 'ZodNullable') {
      nullable = true;
      current = current._def.innerType;
      description ||= getSchemaDescription(current);
      continue;
    }
    if (typeName === 'ZodDefault') {
      optional = true;
      defaultValue =
        typeof current._def.defaultValue === 'function'
          ? current._def.defaultValue()
          : current._def.defaultValue;
      current = current._def.innerType;
      description ||= getSchemaDescription(current);
      continue;
    }
    if (typeName === 'ZodEffects') {
      current = current._def.schema;
      description ||= getSchemaDescription(current);
      continue;
    }
    break;
  }

  return {
    schema: current,
    optional,
    nullable,
    defaultValue,
    description,
  };
}

function objectShape(schema) {
  if (!schema?._def) return {};
  if (typeof schema._def.shape === 'function') return schema._def.shape();
  if (schema.shape) return schema.shape;
  return schema._def.shape || {};
}

function applyStringChecks(target, checks = []) {
  for (const check of checks) {
    if (!check || typeof check !== 'object') continue;
    if (check.kind === 'min') target.minLength = check.value;
    if (check.kind === 'max') target.maxLength = check.value;
    if (check.kind === 'email') target.format = 'email';
    if (check.kind === 'url') target.format = 'uri';
    if (check.kind === 'uuid') target.format = 'uuid';
    if (check.kind === 'datetime') target.format = 'date-time';
    if (check.kind === 'regex' && check.regex instanceof RegExp) {
      target.pattern = check.regex.source;
    }
  }
}

function applyNumberChecks(target, checks = []) {
  for (const check of checks) {
    if (!check || typeof check !== 'object') continue;
    if (check.kind === 'int') target.type = 'integer';
    if (check.kind === 'min') target.minimum = check.value;
    if (check.kind === 'max') target.maximum = check.value;
    if (check.kind === 'multipleOf') target.multipleOf = check.value;
  }
}

function convertSchema(schema) {
  const metadata = unwrapSchema(schema);
  const inner = metadata.schema;
  const typeName = inner?._def?.typeName;
  const description = metadata.description || getSchemaDescription(inner);

  let jsonSchema;

  switch (typeName) {
    case 'ZodString': {
      jsonSchema = { type: 'string' };
      applyStringChecks(jsonSchema, inner._def.checks);
      break;
    }
    case 'ZodNumber': {
      jsonSchema = { type: 'number' };
      applyNumberChecks(jsonSchema, inner._def.checks);
      break;
    }
    case 'ZodBoolean': {
      jsonSchema = { type: 'boolean' };
      break;
    }
    case 'ZodEnum': {
      jsonSchema = {
        type: 'string',
        enum: Array.isArray(inner._def.values) ? inner._def.values : [],
      };
      break;
    }
    case 'ZodNativeEnum': {
      const values = Object.values(inner._def.values || {}).filter(
        (value, index, list) =>
          (typeof value === 'string' || typeof value === 'number') && list.indexOf(value) === index,
      );
      jsonSchema = {
        enum: values,
      };
      break;
    }
    case 'ZodLiteral': {
      jsonSchema = {
        const: inner._def.value,
      };
      break;
    }
    case 'ZodArray': {
      const item = convertSchema(inner._def.type);
      jsonSchema = {
        type: 'array',
        items: item.jsonSchema,
      };
      for (const check of inner._def.exactLength
        ? [inner._def.exactLength]
        : inner._def.minLength
          ? [inner._def.minLength]
          : []) {
        if (check?.value !== undefined) {
          jsonSchema.minItems = check.value;
          jsonSchema.maxItems = check.value;
        }
      }
      if (inner._def.minLength?.value !== undefined)
        jsonSchema.minItems = inner._def.minLength.value;
      if (inner._def.maxLength?.value !== undefined)
        jsonSchema.maxItems = inner._def.maxLength.value;
      break;
    }
    case 'ZodObject': {
      const shape = objectShape(inner);
      const properties = {};
      const required = [];
      for (const [key, value] of Object.entries(shape)) {
        const property = convertSchema(value);
        properties[key] = property.jsonSchema;
        if (!property.optional) required.push(key);
      }
      jsonSchema = {
        type: 'object',
        properties,
        additionalProperties: false,
      };
      if (required.length > 0) {
        jsonSchema.required = required;
      }
      break;
    }
    case 'ZodRecord': {
      const valueType = inner._def.valueType || z.unknown();
      jsonSchema = {
        type: 'object',
        additionalProperties: convertSchema(valueType).jsonSchema,
      };
      break;
    }
    case 'ZodUnion': {
      jsonSchema = {
        anyOf: (inner._def.options || []).map((option) => convertSchema(option).jsonSchema),
      };
      break;
    }
    case 'ZodTuple': {
      const items = (inner._def.items || []).map((item) => convertSchema(item).jsonSchema);
      jsonSchema = {
        type: 'array',
        prefixItems: items,
        minItems: items.length,
        maxItems: items.length,
      };
      break;
    }
    case 'ZodUnknown':
    case 'ZodAny': {
      jsonSchema = {};
      break;
    }
    case 'ZodNull': {
      jsonSchema = { type: 'null' };
      break;
    }
    case 'ZodDate': {
      jsonSchema = {
        type: 'string',
        format: 'date-time',
      };
      break;
    }
    default: {
      jsonSchema = {};
      break;
    }
  }

  return {
    jsonSchema: withMetadata(jsonSchema, {
      description,
      defaultValue: metadata.defaultValue,
      nullable: metadata.nullable,
    }),
    optional: metadata.optional,
  };
}

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
  return convertSchema(createToolInputSchema(inputSchema)).jsonSchema;
}
