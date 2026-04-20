/**
 * Fulfillment Commands Module
 */

function parseIntArg(value, usage) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed < 0) {
    throw new Error(usage);
  }
  return parsed;
}

function parseOrderIds(value) {
  return String(value)
    .split(',')
    .map((id) => id.trim())
    .filter(Boolean);
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'waves': {
      const waves = await commerce.fulfillment.listWaves();
      return formatWaves(waves, { output, jsonOutput });
    }

    case 'wave': {
      const [waveId] = args;
      if (!waveId) throw new Error('Usage: fulfillment wave <waveId>');
      const wave = await commerce.fulfillment.getWave(waveId);
      if (!wave) throw new Error(`Fulfillment wave not found: ${waveId}`);
      return formatWave(wave, { jsonOutput });
    }

    case 'create-wave': {
      const [warehouseIdRaw, orderIdsCsv, priorityRaw, ...noteParts] = args;
      if (!warehouseIdRaw || !orderIdsCsv) {
        throw new Error(
          'Usage: fulfillment create-wave <warehouseId> <orderIdsCsv> [priority] [notes]',
        );
      }
      const wave = await commerce.fulfillment.createWave({
        warehouseId: parseIntArg(
          warehouseIdRaw,
          'Usage: fulfillment create-wave <warehouseId> <orderIdsCsv> [priority] [notes]',
        ),
        orderIds: parseOrderIds(orderIdsCsv),
        priority: priorityRaw ? parseIntArg(priorityRaw, 'priority must be an integer') : undefined,
        notes: noteParts.join(' ') || undefined,
      });
      return {
        wave,
        formatted: `Created fulfillment wave ${wave.id}`,
      };
    }

    case 'release-wave': {
      const [waveId] = args;
      if (!waveId) throw new Error('Usage: fulfillment release-wave <waveId>');
      const wave = await commerce.fulfillment.releaseWave(waveId);
      return {
        wave,
        formatted: `Released fulfillment wave ${wave.id}`,
      };
    }

    case 'complete-wave': {
      const [waveId] = args;
      if (!waveId) throw new Error('Usage: fulfillment complete-wave <waveId>');
      const wave = await commerce.fulfillment.completeWave(waveId);
      return {
        wave,
        formatted: `Completed fulfillment wave ${wave.id}`,
      };
    }

    case 'cancel-wave': {
      const [waveId] = args;
      if (!waveId) throw new Error('Usage: fulfillment cancel-wave <waveId>');
      const wave = await commerce.fulfillment.cancelWave(waveId);
      return {
        wave,
        formatted: `Cancelled fulfillment wave ${wave.id}`,
      };
    }

    case 'picks': {
      const picks = await commerce.fulfillment.listPicks();
      return formatPicks(picks, { output, jsonOutput });
    }

    case 'pick': {
      const [pickId] = args;
      if (!pickId) throw new Error('Usage: fulfillment pick <pickId>');
      const pick = await commerce.fulfillment.getPick(pickId);
      if (!pick) throw new Error(`Pick task not found: ${pickId}`);
      return formatPick(pick, { jsonOutput });
    }

    case 'assign-pick': {
      const [pickId, assignedTo] = args;
      if (!pickId || !assignedTo) {
        throw new Error('Usage: fulfillment assign-pick <pickId> <assignedTo>');
      }
      const pick = await commerce.fulfillment.assignPick(pickId, assignedTo);
      return {
        pick,
        formatted: `Assigned pick task ${pick.id} to ${assignedTo}`,
      };
    }

    case 'start-pick': {
      const [pickId] = args;
      if (!pickId) throw new Error('Usage: fulfillment start-pick <pickId>');
      const pick = await commerce.fulfillment.startPick(pickId);
      return {
        pick,
        formatted: `Started pick task ${pick.id}`,
      };
    }

    case 'cancel-pick': {
      const [pickId] = args;
      if (!pickId) throw new Error('Usage: fulfillment cancel-pick <pickId>');
      const pick = await commerce.fulfillment.cancelPick(pickId);
      return {
        pick,
        formatted: `Cancelled pick task ${pick.id}`,
      };
    }

    case 'ready-pack': {
      const [orderId] = args;
      if (!orderId) throw new Error('Usage: fulfillment ready-pack <orderId>');
      const readyToPack = await commerce.fulfillment.isOrderReadyToPack(orderId);
      return jsonOutput
        ? { orderId, readyToPack }
        : { formatted: `Order ${orderId} ready to pack: ${readyToPack ? 'yes' : 'no'}` };
    }

    case 'ready-ship': {
      const [orderId] = args;
      if (!orderId) throw new Error('Usage: fulfillment ready-ship <orderId>');
      const readyToShip = await commerce.fulfillment.isOrderReadyToShip(orderId);
      return jsonOutput
        ? { orderId, readyToShip }
        : { formatted: `Order ${orderId} ready to ship: ${readyToShip ? 'yes' : 'no'}` };
    }

    case 'count': {
      const count = await commerce.fulfillment.countWaves();
      return { count, formatted: `Fulfillment wave count: ${count}` };
    }

    default:
      throw new Error(
        `Unknown action: fulfillment ${action}\n\n` +
          'Available actions:\n' +
          '  waves                                                              List fulfillment waves\n' +
          '  wave <waveId>                                                      Get fulfillment wave\n' +
          '  create-wave <warehouseId> <orderIdsCsv> [priority] [notes]        Create fulfillment wave\n' +
          '  release-wave <waveId>                                              Release fulfillment wave\n' +
          '  complete-wave <waveId>                                             Complete fulfillment wave\n' +
          '  cancel-wave <waveId>                                               Cancel fulfillment wave\n' +
          '  picks                                                              List pick tasks\n' +
          '  pick <pickId>                                                      Get pick task\n' +
          '  assign-pick <pickId> <assignedTo>                                  Assign pick task\n' +
          '  start-pick <pickId>                                                Start pick task\n' +
          '  cancel-pick <pickId>                                               Cancel pick task\n' +
          '  ready-pack <orderId>                                               Check order ready to pack\n' +
          '  ready-ship <orderId>                                               Check order ready to ship\n' +
          '  count                                                              Count fulfillment waves',
      );
  }
}

function formatWaves(waves, { output, jsonOutput }) {
  if (jsonOutput) return waves;
  if (waves.length === 0) return { formatted: 'No fulfillment waves found.' };
  const formatted = output.table(waves, [
    { key: 'id', header: 'ID' },
    { key: 'warehouseId', header: 'Warehouse', align: 'right' },
    { key: 'status', header: 'Status' },
    { key: 'priority', header: 'Priority', align: 'right' },
    { key: 'orderCount', header: 'Orders', align: 'right' },
  ]);
  return { waves, formatted };
}

function formatWave(wave, { jsonOutput }) {
  if (jsonOutput) return wave;
  return {
    wave,
    formatted:
      `Fulfillment wave: ${wave.id}\n` +
      `${'-'.repeat(40)}\n` +
      `Warehouse:     ${wave.warehouseId}\n` +
      `Status:        ${wave.status}\n` +
      `Priority:      ${wave.priority ?? 'N/A'}\n` +
      `Orders:        ${wave.orderCount ?? wave.orderIds?.length ?? 0}\n` +
      `Notes:         ${wave.notes || 'N/A'}`,
  };
}

function formatPicks(picks, { output, jsonOutput }) {
  if (jsonOutput) return picks;
  if (picks.length === 0) return { formatted: 'No pick tasks found.' };
  const formatted = output.table(picks, [
    { key: 'id', header: 'ID' },
    { key: 'waveId', header: 'Wave' },
    { key: 'assignedTo', header: 'Assigned To' },
    { key: 'status', header: 'Status' },
    { key: 'sku', header: 'SKU' },
    { key: 'quantity', header: 'Qty', align: 'right' },
  ]);
  return { picks, formatted };
}

function formatPick(pick, { jsonOutput }) {
  if (jsonOutput) return pick;
  return {
    pick,
    formatted:
      `Pick task: ${pick.id}\n` +
      `${'-'.repeat(34)}\n` +
      `Wave:          ${pick.waveId}\n` +
      `Assigned to:   ${pick.assignedTo || 'N/A'}\n` +
      `Status:        ${pick.status}\n` +
      `SKU:           ${pick.sku || 'N/A'}\n` +
      `Quantity:      ${pick.quantity ?? 'N/A'}`,
  };
}

export const metadata = {
  name: 'fulfillment',
  aliases: ['fulfill', 'pick'],
  description: 'Fulfillment waves and pick task commands',
  actions: {
    waves: { description: 'List fulfillment waves', args: [] },
    wave: { description: 'Get fulfillment wave', args: ['<waveId>'] },
    'create-wave': {
      description: 'Create fulfillment wave',
      args: ['<warehouseId>', '<orderIdsCsv>', '[priority]', '[notes]'],
    },
    'release-wave': { description: 'Release fulfillment wave', args: ['<waveId>'] },
    'complete-wave': { description: 'Complete fulfillment wave', args: ['<waveId>'] },
    'cancel-wave': { description: 'Cancel fulfillment wave', args: ['<waveId>'] },
    picks: { description: 'List pick tasks', args: [] },
    pick: { description: 'Get pick task', args: ['<pickId>'] },
    'assign-pick': { description: 'Assign pick task', args: ['<pickId>', '<assignedTo>'] },
    'start-pick': { description: 'Start pick task', args: ['<pickId>'] },
    'cancel-pick': { description: 'Cancel pick task', args: ['<pickId>'] },
    'ready-pack': { description: 'Check order ready to pack', args: ['<orderId>'] },
    'ready-ship': { description: 'Check order ready to ship', args: ['<orderId>'] },
    count: { description: 'Count fulfillment waves', args: [] },
  },
};

export default { execute, metadata };
