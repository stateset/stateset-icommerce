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

/** The engine caps a single `segments.list` page at 1000 rows (default 500). */
const SEGMENT_PAGE_SIZE = 1000;

/** Every segment matching `filter`, paging past the engine's per-call limit. */
async function listAllSegments(commerce, filter) {
  const all = [];
  for (let offset = 0; ; offset += SEGMENT_PAGE_SIZE) {
    const page = await commerce.segments.list({ ...filter, limit: SEGMENT_PAGE_SIZE, offset });
    all.push(...page);
    if (page.length < SEGMENT_PAGE_SIZE) return all;
  }
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'list': {
      const [type, limitRaw] = args;
      const segments = await listAllSegments(commerce, { segmentType: type || undefined });
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
      // Stored membership: nothing in the engine evaluates segment rules, so this
      // reports whether the customer is recorded as a member.
      const isMember = await commerce.segments.isMember(segmentId, customerId);
      return formatEvaluation(
        segmentId,
        customerId,
        { isMember, basis: 'stored_membership' },
        { jsonOutput },
      );
    }

    case 'count': {
      const type = args[0];
      const count = (await listAllSegments(commerce, { segmentType: type || undefined })).length;
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
          '  evaluate <segmentId> <customerId>   Check stored customer membership\n' +
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
    { key: 'segmentType', header: 'Type' },
    { key: 'memberCount', header: 'Members', align: 'right' },
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
      `Type:         ${segment.segmentType}\n` +
      `Members:      ${segment.memberCount}\n` +
      `Rules:        ${Array.isArray(segment.rules) ? segment.rules.length : 0} (all must match)`,
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
      `Is member:    ${result.isMember ? 'yes' : 'no'} (stored membership)`,
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
      description: 'Check stored customer membership',
      args: ['<segmentId>', '<customerId>'],
    },
    count: { description: 'Count segments', args: ['[type]'] },
  },
};

export default { execute, metadata };
