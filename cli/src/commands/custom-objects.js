/**
 * Custom Objects Commands Module
 */

function parseJsonArg(value, label) {
  try {
    return JSON.parse(value);
  } catch (error) {
    throw new Error(`Invalid ${label} JSON: ${error.message}`);
  }
}

function normalizeValuesJson({ values, valuesJson }) {
  if (typeof valuesJson === 'string' && valuesJson.trim().length > 0) return valuesJson;
  if (values && typeof values === 'object') return JSON.stringify(values);
  return '{}';
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'types': {
      const [search, limitRaw, offsetRaw] = args;
      const types = await commerce.customObjects.listTypes({
        search: search || undefined,
        limit: limitRaw ? Number.parseInt(limitRaw, 10) : undefined,
        offset: offsetRaw ? Number.parseInt(offsetRaw, 10) : undefined,
      });
      return formatTypeList(types, { output, jsonOutput });
    }

    case 'type': {
      const id = args[0];
      if (!id) throw new Error('Usage: custom-objects type <id>');
      const type = await commerce.customObjects.getType(id);
      if (!type) throw new Error(`Custom object type not found: ${id}`);
      return formatTypeDetail(type, { jsonOutput });
    }

    case 'type-handle': {
      const handle = args[0];
      if (!handle) throw new Error('Usage: custom-objects type-handle <handle>');
      const type = await commerce.customObjects.getTypeByHandle(handle);
      if (!type) throw new Error(`Custom object type not found: ${handle}`);
      return formatTypeDetail(type, { jsonOutput });
    }

    case 'create-type': {
      const payloadJson = args[0];
      if (!payloadJson) throw new Error('Usage: custom-objects create-type <payloadJson>');
      const type = await commerce.customObjects.createType(parseJsonArg(payloadJson, 'payload'));
      return { type, formatted: `Created custom object type ${type.handle || type.id}` };
    }

    case 'update-type': {
      const [id, payloadJson] = args;
      if (!id || !payloadJson) {
        throw new Error('Usage: custom-objects update-type <id> <payloadJson>');
      }
      const type = await commerce.customObjects.updateType(
        id,
        parseJsonArg(payloadJson, 'payload'),
      );
      return { type, formatted: `Updated custom object type ${type.handle || type.id}` };
    }

    case 'delete-type': {
      const id = args[0];
      if (!id) throw new Error('Usage: custom-objects delete-type <id>');
      await commerce.customObjects.deleteType(id);
      return { formatted: `Deleted custom object type ${id}` };
    }

    case 'list': {
      const [typeHandle, ownerType, ownerId, handle, limitRaw, offsetRaw] = args;
      const objects = await commerce.customObjects.listObjects({
        typeHandle: typeHandle || undefined,
        ownerType: ownerType || undefined,
        ownerId: ownerId || undefined,
        handle: handle || undefined,
        limit: limitRaw ? Number.parseInt(limitRaw, 10) : undefined,
        offset: offsetRaw ? Number.parseInt(offsetRaw, 10) : undefined,
      });
      return formatObjectList(objects, { output, jsonOutput });
    }

    case 'get': {
      const id = args[0];
      if (!id) throw new Error('Usage: custom-objects get <id>');
      const object = await commerce.customObjects.getObject(id);
      if (!object) throw new Error(`Custom object not found: ${id}`);
      return formatObjectDetail(object, { jsonOutput });
    }

    case 'handle': {
      const [typeHandle, objectHandle] = args;
      if (!typeHandle || !objectHandle) {
        throw new Error('Usage: custom-objects handle <typeHandle> <objectHandle>');
      }
      const object = await commerce.customObjects.getObjectByHandle(typeHandle, objectHandle);
      if (!object) throw new Error(`Custom object not found: ${typeHandle}/${objectHandle}`);
      return formatObjectDetail(object, { jsonOutput });
    }

    case 'create': {
      const payloadJson = args[0];
      if (!payloadJson) throw new Error('Usage: custom-objects create <payloadJson>');
      const payload = parseJsonArg(payloadJson, 'payload');
      const object = await commerce.customObjects.createObject({
        typeHandle: payload.typeHandle,
        handle: payload.handle,
        ownerType: payload.ownerType,
        ownerId: payload.ownerId,
        valuesJson: normalizeValuesJson(payload),
      });
      return { object, formatted: `Created custom object ${object.handle || object.id}` };
    }

    case 'update': {
      const [id, payloadJson] = args;
      if (!id || !payloadJson) throw new Error('Usage: custom-objects update <id> <payloadJson>');
      const payload = parseJsonArg(payloadJson, 'payload');
      const object = await commerce.customObjects.updateObject(id, {
        handle: payload.handle,
        ownerType: payload.ownerType,
        ownerId: payload.ownerId,
        valuesJson:
          typeof payload.valuesJson === 'string'
            ? payload.valuesJson
            : payload.values
              ? JSON.stringify(payload.values)
              : undefined,
      });
      return { object, formatted: `Updated custom object ${object.handle || object.id}` };
    }

    case 'delete': {
      const id = args[0];
      if (!id) throw new Error('Usage: custom-objects delete <id>');
      await commerce.customObjects.deleteObject(id);
      return { formatted: `Deleted custom object ${id}` };
    }

    default:
      throw new Error(
        `Unknown action: custom-objects ${action}\n\n` +
          'Available actions:\n' +
          '  types [search] [limit] [offset]                     List custom object types\n' +
          '  type <id>                                           Get custom object type by ID\n' +
          '  type-handle <handle>                                Get custom object type by handle\n' +
          '  create-type <payloadJson>                           Create custom object type\n' +
          '  update-type <id> <payloadJson>                      Update custom object type\n' +
          '  delete-type <id>                                    Delete custom object type\n' +
          '  list [typeHandle] [ownerType] [ownerId] [handle] [limit] [offset]  List custom objects\n' +
          '  get <id>                                            Get custom object by ID\n' +
          '  handle <typeHandle> <objectHandle>                  Get custom object by handle\n' +
          '  create <payloadJson>                                Create custom object\n' +
          '  update <id> <payloadJson>                           Update custom object\n' +
          '  delete <id>                                         Delete custom object',
      );
  }
}

function formatTypeList(types, { output, jsonOutput }) {
  if (jsonOutput) return types;
  if (types.length === 0) return { formatted: 'No custom object types found.' };
  const formatted = output.table(types, [
    { key: 'id', header: 'ID' },
    { key: 'handle', header: 'Handle' },
    { key: 'displayName', header: 'Name' },
    { key: 'description', header: 'Description' },
  ]);
  return { types, formatted };
}

function formatTypeDetail(type, { jsonOutput }) {
  if (jsonOutput) return type;
  return {
    type,
    formatted:
      `Custom object type: ${type.displayName || type.handle}\n` +
      `${'-'.repeat(42)}\n` +
      `ID:           ${type.id}\n` +
      `Handle:       ${type.handle}\n` +
      `Description:  ${type.description || 'N/A'}\n` +
      `Fields:       ${Array.isArray(type.fields) ? type.fields.length : 0}`,
  };
}

function formatObjectList(objects, { output, jsonOutput }) {
  if (jsonOutput) return objects;
  if (objects.length === 0) return { formatted: 'No custom objects found.' };
  const formatted = output.table(objects, [
    { key: 'id', header: 'ID' },
    { key: 'typeHandle', header: 'Type' },
    { key: 'handle', header: 'Handle' },
    { key: 'ownerType', header: 'Owner Type' },
    { key: 'ownerId', header: 'Owner ID' },
  ]);
  return { objects, formatted };
}

function formatObjectDetail(object, { jsonOutput }) {
  if (jsonOutput) return object;
  return {
    object,
    formatted:
      `Custom object: ${object.handle || object.id}\n` +
      `${'-'.repeat(38)}\n` +
      `ID:           ${object.id}\n` +
      `Type:         ${object.typeHandle}\n` +
      `Owner:        ${object.ownerType || 'N/A'}:${object.ownerId || 'N/A'}\n` +
      `Values:       ${object.valuesJson || JSON.stringify(object.values || {})}`,
  };
}

export const metadata = {
  name: 'custom-objects',
  aliases: ['co', 'metaobjects'],
  description: 'Custom object schema and record commands',
  actions: {
    types: { description: 'List custom object types', args: ['[search]', '[limit]', '[offset]'] },
    type: { description: 'Get custom object type', args: ['<id>'] },
    'type-handle': { description: 'Get custom object type by handle', args: ['<handle>'] },
    'create-type': { description: 'Create custom object type', args: ['<payloadJson>'] },
    'update-type': { description: 'Update custom object type', args: ['<id>', '<payloadJson>'] },
    'delete-type': { description: 'Delete custom object type', args: ['<id>'] },
    list: {
      description: 'List custom objects',
      args: ['[typeHandle]', '[ownerType]', '[ownerId]', '[handle]', '[limit]', '[offset]'],
    },
    get: { description: 'Get custom object', args: ['<id>'] },
    handle: {
      description: 'Get custom object by handle',
      args: ['<typeHandle>', '<objectHandle>'],
    },
    create: { description: 'Create custom object', args: ['<payloadJson>'] },
    update: { description: 'Update custom object', args: ['<id>', '<payloadJson>'] },
    delete: { description: 'Delete custom object', args: ['<id>'] },
  },
};

export default { execute, metadata };
