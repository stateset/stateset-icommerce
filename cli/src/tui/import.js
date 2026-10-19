import fs from 'node:fs/promises';
import { constants } from 'node:fs';
import path from 'node:path';
import { scenarioSettings, digestInputs } from './workspace.js';
import { simulationPlans } from './model.js';
import { SIMULATION_KERNEL_VERSION } from '../simulation/limits.js';

const MAX_BYTES = 8 * 1024 * 1024;

// Only bounded assumptions are imported. File-provided reports are never
// presented as locally executed evidence, and imports never dispatch a run.
export function importedScenario(value, filename = 'export.json') {
  if (
    !value ||
    value.format !== 'stateset-tui-run' ||
    value.version !== 1 ||
    value.kernelVersion !== SIMULATION_KERNEL_VERSION
  )
    throw new Error('Unsupported run export or kernel version');
  if (!value.controls || !Array.isArray(value.products))
    throw new Error('Export is missing planning assumptions');
  const settings = scenarioSettings(value);
  const expected = simulationPlans(
    settings.presetId,
    settings.seed,
    settings.controls,
    settings.products,
  );
  if (
    !Array.isArray(value.result?.inputs) ||
    JSON.stringify(expected) !== JSON.stringify(value.result.inputs)
  )
    throw new Error('Export assumptions differ from executed inputs');
  if (
    typeof value.inputDigest !== 'string' ||
    value.inputDigest !== digestInputs(value.result.inputs)
  )
    throw new Error('Export input digest does not match');
  return {
    settings,
    filename: path.basename(filename),
    inputDigest: value.inputDigest,
    cliVersion: typeof value.cliVersion === 'string' ? value.cliVersion.slice(0, 80) : 'unknown',
  };
}

export async function readScenarioExport(filename) {
  if (typeof filename !== 'string' || !filename.trim())
    throw new Error('Enter a local JSON filename');
  const handle = await fs.open(
    path.resolve(filename.trim()),
    constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK,
  );
  try {
    const stat = await handle.stat();
    if (!stat.isFile() || stat.size > MAX_BYTES)
      throw new Error('Import needs a regular JSON file, at most 8 MiB');
    const buffer = Buffer.alloc(MAX_BYTES + 1);
    let length = 0;
    while (length < buffer.length) {
      const { bytesRead } = await handle.read(buffer, length, buffer.length - length, null);
      if (!bytesRead) break;
      length += bytesRead;
    }
    if (length > MAX_BYTES) throw new Error('Import exceeds 8 MiB');
    let value;
    try {
      value = JSON.parse(buffer.subarray(0, length).toString('utf8'));
    } catch {
      throw new Error('Import is not valid JSON');
    }
    return importedScenario(value, filename);
  } finally {
    await handle.close();
  }
}
