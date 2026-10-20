import { DOMAINS, domainById, explorerRows, explorerRequestKey, tableLabel } from './domains.js';
import { fit, wrap } from './text.js';

function summary(row) {
  const title = [
    'name',
    'order_number',
    'po_number',
    'invoice_number',
    'shipment_number',
    'bill_number',
    'sku',
    'email',
    'account_number',
    'code',
    'id',
  ].find((k) => row[k]);
  const secondary = [
    'status',
    'currency',
    'total',
    'total_amount',
    'amount',
    'current_balance',
    'available',
    'created_at',
  ].filter((k) => row[k] !== null && row[k] !== undefined);
  return `${title ? row[title] : Object.values(row).find((v) => v !== null) || '(record)'}  ${secondary.map((k) => `${k}: ${row[k]}`).join(' · ')}`;
}

export function explorerBody(state, size, available, compact = false) {
  const body = [],
    e = state.explorer,
    domain = domainById(e.domain);
  const put = (text = '', tone = 'ink', bold = false) => body.push({ text, tone, bold });
  const info = (text) => wrap(text, size).forEach((line) => put(line, 'muted'));
  const rows = explorerRows(state);
  let headers = compact ? 1 : 3;
  if (e.mode === 'domains') {
    put(
      compact ? 'DOMAINS' : `ENGINE EXPLORER · ${DOMAINS.length - 1} domains + runtime`,
      'accent',
      true,
    );
    if (!compact) {
      put('Enter resources · / filter · [ ] groups · : jump', 'muted');
      put(`${rows.length} matches · names, groups and tiers`, 'muted');
    }
  } else if (e.mode === 'resources') {
    put(`${domain.name.toUpperCase()} · ${domain.group}`, 'accent', true);
    if (!compact) {
      put(`${domain.tier} tools · T reference · d domains · b back`, 'muted');
      put('Enter opens stored records from the private engine snapshot.', 'muted');
    }
    if (!rows.length) {
      if (compact) {
        put(e.query ? 'No matches. Esc' : 'Reference only', 'muted');
        put(domain.id === 'simulations' ? 'L simulation lab' : 'T tools · b back', 'muted');
        return body;
      }
      info(
        e.query
          ? 'No resources match this filter. Esc clears it.'
          : 'No local table browser is registered for this domain. T opens the tool reference.',
      );
      if (domain.reason) info(domain.reason);
      if (domain.id === 'simulations')
        info('L opens the working simulation lab, including saved scenarios and comparisons.');
      else info('Tool reference describes capabilities; this TUI does not execute those tools.');
      return body.slice(0, available);
    }
  } else if (e.mode === 'tools') {
    put(
      compact ? 'TOOLS · reference' : `${domain.name.toUpperCase()} · TOOL REFERENCE`,
      'accent',
      true,
    );
    if (!compact) {
      put('Documentation only · Enter details · / filter · b back', 'muted');
      put(`${rows.length} tools · read/write permissions and stability tiers`, 'muted');
    }
  } else if (e.mode === 'links') {
    put('LINKED RECORDS', 'accent', true);
    if (!compact) {
      put('Enter follows the selected relationship · b returns', 'muted');
      put('Engine foreign keys and documented parent identifiers', 'muted');
    }
  } else {
    put(`${domain.name.toUpperCase()} / ${tableLabel(e.table)}`, 'accent', true);
    const result = state.data?.explorer;
    if (result?.requestKey !== explorerRequestKey(e)) {
      put(state.loading ? 'Loading resource…' : 'r loads this resource', 'muted');
      return body;
    }
    if (result.unavailable) {
      wrap(result.unavailable, size).forEach((line) => put(line, 'warn'));
      put('r retry · T tool reference · b resources · d domains', 'muted');
      return body.slice(0, available);
    }
    if (!compact) {
      put(
        `Page ${e.page + 1} · ${rows.length} shown / ${result.matched} matches · n/p · o sort`,
        'muted',
      );
      put(
        `Enter details · t links · / search all · o ${e.sort === 1 ? 'Name/ID' : 'Recent/ID'}`,
        'muted',
      );
      if (e.relation) {
        put(`Linked ${e.relation.column} = ${e.relation.value}`, 'muted');
        headers++;
      }
    }
  }
  const visible = Math.max(1, available - headers - 1),
    start = Math.max(0, state.selected - visible + 1);
  rows.slice(start, start + visible).forEach((row, i) => {
    let label;
    if (e.mode === 'domains')
      label = compact
        ? row.name
        : `${fit(row.name, Math.min(25, Math.floor(size / 2)))} ${row.group} · ${row.tables.length ? 'records' : 'reference'} · ${row.tier}`;
    else if (e.mode === 'resources' || e.mode === 'links') label = row.name;
    else if (e.mode === 'tools') label = `${row.name} · ${row.permission} · ${row.tier}`;
    else label = summary(row);
    put(
      `${i + start === state.selected ? '›' : ' '} ${label}`,
      i + start === state.selected ? 'accent' : 'ink',
      i + start === state.selected,
    );
  });
  if (!rows.length) {
    const kind = { domains: 'domains', tools: 'tools', links: 'links' }[e.mode] || 'records';
    put(`No matching ${kind}. Esc clears the filter.`, 'muted');
  }
  if (e.mode === 'records' && !compact)
    put('Stored values · no derived totals · 4096 chars/field · T tools', 'muted');
  else put(`${rows.length ? state.selected + 1 : 0}/${rows.length} · ↑ ↓ / Home End`, 'muted');
  return body;
}

export function explorerDetail(state) {
  const record = explorerRows(state)[state.selected];
  if (!record) return record;
  if (state.explorer.mode === 'tools')
    return { ...record, execution: 'Reference only. No tool is invoked by this workbench.' };
  return { ...record, _inspection: state.data?.explorer?.warnings || [] };
}
