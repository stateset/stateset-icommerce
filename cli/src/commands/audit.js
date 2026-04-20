/**
 * Audit Commands Module
 */

function parseLimit(value, usage) {
  if (value === undefined) return undefined;
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed <= 0) throw new Error(usage);
  return parsed;
}

export async function execute(action, args, { output, jsonOutput }) {
  const { getAuditStore } = await import('../audit-store.js');
  const store = getAuditStore();

  switch (action) {
    case 'query': {
      const [tool, result, since, limitRaw] = args;
      const usage = 'Usage: audit query [tool] [result] [since] [limit]';
      const entries = store.query({
        tool: tool || null,
        result: result || null,
        since: since || null,
        limit: parseLimit(limitRaw, usage) || 50,
      });
      return formatAuditEntries(entries, { output, jsonOutput });
    }

    case 'summary': {
      const since = args[0];
      const entries = store.query({
        since: since || null,
        limit: 10000,
      });
      const byResult = {};
      const byTool = {};
      for (const entry of entries) {
        byResult[entry.result] = (byResult[entry.result] || 0) + 1;
        byTool[entry.tool] = (byTool[entry.tool] || 0) + 1;
      }
      const topTools = Object.entries(byTool)
        .sort((left, right) => right[1] - left[1])
        .slice(0, 10)
        .map(([tool, count]) => ({ tool, count }));
      const summary = {
        totalEntries: store.count(),
        queriedEntries: entries.length,
        since: since || null,
        byResult,
        topTools,
      };
      return formatAuditSummary(summary, { output, jsonOutput });
    }

    case 'export': {
      const [since, limitRaw, format = 'json'] = args;
      const usage = 'Usage: audit export [since] [limit] [format]';
      const exported = store.export({
        since: since || null,
        limit: parseLimit(limitRaw, usage) || 10000,
      });

      if (format === 'csv') {
        const headers = 'id,timestamp,tool,result,reason,level,session_id,agent';
        const rows = exported.entries.map((entry) =>
          [
            entry.id,
            entry.timestamp,
            entry.tool,
            entry.result,
            (entry.reason || '').replaceAll(',', ';'),
            entry.level,
            entry.session_id || '',
            entry.agent || '',
          ].join(','),
        );
        const csv = [headers, ...rows].join('\n');
        return jsonOutput
          ? { ...exported, format: 'csv', csv }
          : {
              exported,
              csv,
              formatted:
                `Audit export\n` +
                `${'-'.repeat(22)}\n` +
                `Format:      csv\n` +
                `Entries:     ${exported.entries.length}\n` +
                `Exported:    ${exported.exportedAt}`,
            };
      }

      return jsonOutput
        ? { ...exported, format: 'json' }
        : {
            exported,
            formatted:
              `Audit export\n` +
              `${'-'.repeat(22)}\n` +
              `Format:      json\n` +
              `Entries:     ${exported.entries.length}\n` +
              `Exported:    ${exported.exportedAt}`,
          };
    }

    case 'retention': {
      const beforeCount = store.count();
      store.cleanup();
      const afterCount = store.count();
      return {
        entriesBefore: beforeCount,
        entriesAfter: afterCount,
        entriesRemoved: beforeCount - afterCount,
        formatted:
          `Audit retention cleanup complete\n` +
          `${'-'.repeat(38)}\n` +
          `Before:   ${beforeCount}\n` +
          `After:    ${afterCount}\n` +
          `Removed:  ${beforeCount - afterCount}`,
      };
    }

    default:
      throw new Error(
        `Unknown action: audit ${action}\n\n` +
          'Available actions:\n' +
          '  query [tool] [result] [since] [limit]  Query audit log\n' +
          '  summary [since]                        Summarize audit activity\n' +
          '  export [since] [limit] [format]        Export audit log\n' +
          '  retention                              Run retention cleanup',
      );
  }
}

function formatAuditEntries(entries, { output, jsonOutput }) {
  if (jsonOutput) return entries;
  if (entries.length === 0) return { formatted: 'No audit entries found.' };
  const formatted = output.table(entries, [
    { key: 'id', header: 'ID' },
    { key: 'timestamp', header: 'Timestamp' },
    { key: 'tool', header: 'Tool' },
    { key: 'result', header: 'Result' },
    { key: 'level', header: 'Level' },
  ]);
  return { entries, formatted };
}

function formatAuditSummary(summary, { output, jsonOutput }) {
  if (jsonOutput) return summary;
  const topToolsTable =
    summary.topTools.length === 0
      ? 'No tool activity'
      : output.table(summary.topTools, [
          { key: 'tool', header: 'Tool' },
          { key: 'count', header: 'Count', align: 'right' },
        ]);
  return {
    summary,
    formatted:
      `Audit summary\n` +
      `${'-'.repeat(24)}\n` +
      `Total entries:   ${summary.totalEntries}\n` +
      `Query sample:    ${summary.queriedEntries}\n` +
      `Allowed:         ${summary.byResult.allowed || 0}\n` +
      `Denied:          ${summary.byResult.denied || 0}\n` +
      `Executed:        ${summary.byResult.executed || 0}\n\n` +
      topToolsTable,
  };
}

export const metadata = {
  name: 'audit',
  aliases: ['logs', 'auditlog'],
  description: 'Audit log query and compliance commands',
  actions: {
    query: { description: 'Query audit log', args: ['[tool]', '[result]', '[since]', '[limit]'] },
    summary: { description: 'Summarize audit activity', args: ['[since]'] },
    export: { description: 'Export audit log', args: ['[since]', '[limit]', '[format]'] },
    retention: { description: 'Run retention cleanup', args: [] },
  },
};

export default { execute, metadata };
