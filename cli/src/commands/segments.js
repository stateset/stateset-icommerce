/**
 * Segments Commands Module
 */

function parseJsonArg(value, label) {
  try {
    return JSON.parse(value);
  } catch (error) {
    throw new Error(`Invalid ${label} JSON: ${error.message}`);
  }
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'list': {
      const [type, limitRaw] = args;
      const segments = await commerce.segments.list({ type: type || undefined });
      const limit = limitRaw ? Number.parseInt(limitRaw, 10) : undefined;
      const limited = Number.isInteger(limit) && limit > 0 ? segments.slice(0, limit) : segments;
      return formatSegmentList(limited, { output, jsonOutput });
    }

    case 'get': {
      const segmentId = args[0];
      if (!segmentId) throw new Error('Usage: segments get <segmentId>');
      const segment = await commerce.segments.get(segmentId);
      if (!segment) throw new Error(`Segment not found: ${segmentId}`);
      return formatSegmentDetail(segment, { jsonOutput });
    }

    case 'create': {
      const payloadJson = args[0];
      if (!payloadJson) throw new Error('Usage: segments create <payloadJson>');
      const segment = await commerce.segments.create(parseJsonArg(payloadJson, 'payload'));
      return {
        segment,
        formatted: `Created segment ${segment.id}`,
      };
    }

    case 'update': {
      const [segmentId, updatesJson] = args;
      if (!segmentId || !updatesJson) {
        throw new Error('Usage: segments update <segmentId> <updatesJson>');
      }
      const segment = await commerce.segments.update(
        segmentId,
        parseJsonArg(updatesJson, 'updates'),
      );
      return {
        segment,
        formatted: `Updated segment ${segment.id}`,
      };
    }

    case 'evaluate': {
      const [segmentId, customerId] = args;
      if (!segmentId || !customerId) {
        throw new Error('Usage: segments evaluate <segmentId> <customerId>');
      }
      const result = await commerce.segments.evaluateMembership(segmentId, customerId);
      return formatEvaluation(segmentId, customerId, result, { jsonOutput });
    }

    case 'rebuild': {
      const segmentId = args[0];
      if (!segmentId) throw new Error('Usage: segments rebuild <segmentId>');
      const result = await commerce.segments.rebuild(segmentId);
      return {
        result,
        formatted:
          `Rebuilt segment ${segmentId}\n` +
          `${'-'.repeat(30)}\n` +
          `Members:   ${result.memberCount}\n` +
          `Added:     ${result.added}\n` +
          `Removed:   ${result.removed}`,
      };
    }

    case 'count': {
      const type = args[0];
      const count = await commerce.segments.count({ type: type || undefined });
      return { count, formatted: `Segment count: ${count}` };
    }

    default:
      throw new Error(
        `Unknown action: segments ${action}\n\n` +
          'Available actions:\n' +
          '  list [type] [limit]                 List segments\n' +
          '  get <segmentId>                     Get segment\n' +
          '  create <payloadJson>                Create segment\n' +
          '  update <segmentId> <updatesJson>    Update segment\n' +
          '  evaluate <segmentId> <customerId>   Evaluate customer membership\n' +
          '  rebuild <segmentId>                 Rebuild dynamic segment\n' +
          '  count [type]                        Count segments',
      );
  }
}

function formatSegmentList(segments, { output, jsonOutput }) {
  if (jsonOutput) return segments;
  if (segments.length === 0) return { formatted: 'No segments found.' };
  const formatted = output.table(segments, [
    { key: 'id', header: 'ID' },
    { key: 'name', header: 'Name' },
    { key: 'type', header: 'Type' },
    { key: 'memberCount', header: 'Members', align: 'right' },
    { key: 'status', header: 'Status' },
  ]);
  return { segments, formatted };
}

function formatSegmentDetail(segment, { jsonOutput }) {
  if (jsonOutput) return segment;
  return {
    segment,
    formatted:
      `Segment: ${segment.name}\n` +
      `${'-'.repeat(34)}\n` +
      `ID:           ${segment.id}\n` +
      `Type:         ${segment.type}\n` +
      `Logic:        ${segment.conditionLogic || 'all'}\n` +
      `Members:      ${segment.memberCount ?? 'N/A'}\n` +
      `Status:       ${segment.status || 'N/A'}\n` +
      `Conditions:   ${Array.isArray(segment.conditions) ? segment.conditions.length : 0}`,
  };
}

function formatEvaluation(segmentId, customerId, result, { jsonOutput }) {
  if (jsonOutput) return { segmentId, customerId, ...result };
  return {
    segmentId,
    customerId,
    result,
    formatted:
      `Segment membership\n` +
      `${'-'.repeat(28)}\n` +
      `Segment:      ${segmentId}\n` +
      `Customer:     ${customerId}\n` +
      `Is member:    ${result.isMember ? 'yes' : 'no'}\n` +
      `Matched:      ${Array.isArray(result.matchedConditions) ? result.matchedConditions.length : 0}`,
  };
}

export const metadata = {
  name: 'segments',
  aliases: ['seg', 'segment'],
  description: 'Customer segmentation commands',
  actions: {
    list: { description: 'List segments', args: ['[type]', '[limit]'] },
    get: { description: 'Get segment', args: ['<segmentId>'] },
    create: { description: 'Create segment', args: ['<payloadJson>'] },
    update: { description: 'Update segment', args: ['<segmentId>', '<updatesJson>'] },
    evaluate: {
      description: 'Evaluate customer membership',
      args: ['<segmentId>', '<customerId>'],
    },
    rebuild: { description: 'Rebuild dynamic segment', args: ['<segmentId>'] },
    count: { description: 'Count segments', args: ['[type]'] },
  },
};

export default { execute, metadata };
