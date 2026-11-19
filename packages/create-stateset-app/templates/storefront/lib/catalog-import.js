import { importNextCatalog } from './next-catalog.js';
import { importWhopCatalog } from './whop-catalog.js';

/**
 * @param {any} commerce
 * @param {string} format
 * @param {any} catalog
 * @param {{sourceStore?: string, apply?: boolean, expectedFingerprint?: string}} options
 */
export async function importExternalCatalog(commerce, format, catalog, options) {
  if (format === 'next') return importNextCatalog(commerce, catalog, options);
  if (format !== 'whop') throw new Error('Choose a supported catalog format.');
  return importWhopCatalog(commerce, catalog, options);
}
