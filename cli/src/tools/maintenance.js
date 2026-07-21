/**
 * Maintenance Tools Module
 *
 * MCP tool definitions for database backup, restore, and structured
 * export/import.
 *
 * Two distinct capabilities, deliberately kept separate:
 *
 * - `backup_database` / `restore_database` move a byte-exact database image.
 *   Consistent under concurrent writers (SQLite `VACUUM INTO`), checksum-
 *   verified via a sidecar manifest, and identity-preserving. This is the
 *   disaster-recovery path. SQLite-backed stores only.
 * - `export_full_data` / `import_full_data` move business records as versioned JSON.
 *   Backend-independent and diffable, but IDs are re-minted on import.
 *
 * All three write operations are gated behind `--apply`: restore replaces a
 * database file, import mutates the store, and backup writes to the filesystem
 * outside the store.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

/**
 * Resolve the maintenance accessor, or return a structured error explaining
 * that the running binding predates it.
 *
 * @param {object} commerce
 * @returns {{ ok: true, maintenance: object } | { ok: false, error: object }}
 */
function resolveMaintenance(commerce) {
  const maintenance = commerce?.maintenance;
  if (!maintenance) {
    return {
      ok: false,
      error: {
        success: false,
        error: 'Maintenance operations are not available in this build.',
        hint: 'Upgrade @stateset/embedded to a version that exposes commerce.maintenance.',
      },
    };
  }
  return { ok: true, maintenance };
}

export const maintenanceTools = withPolicyDomain('maintenance', [
  {
    name: 'backup_database',
    description:
      'Create a consistent, checksum-verified backup of the database plus a sidecar manifest. Safe to run while the store is being written to.',
    inputSchema: {
      backupPath: z
        .string()
        .min(1)
        .describe('Destination path for the backup file (a manifest is written alongside it)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Back up database', params);
      }

      const resolved = resolveMaintenance(commerce);
      if (!resolved.ok) {
        return resolved.error;
      }

      const report = await resolved.maintenance.backup(params.backupPath);
      return {
        success: true,
        message: `Database backed up to ${params.backupPath}`,
        backupPath: report?.backupPath ?? params.backupPath,
        manifestPath: report?.manifestPath,
        schemaVersion: report?.manifest?.schemaVersion,
        sizeBytes: report?.manifest?.sizeBytes,
        checksum: report?.manifest?.checksum,
      };
    },
  },
  {
    name: 'restore_database',
    description:
      'Restore a backup to a target path. Verifies the manifest checksum, refuses backups from a newer engine, and refuses to overwrite an existing database unless overwrite is set.',
    inputSchema: {
      backupPath: z.string().min(1).describe('Path to the backup file to restore from'),
      targetPath: z.string().min(1).describe('Path to restore the database to'),
      overwrite: z
        .boolean()
        .optional()
        .describe('Replace an existing non-empty database at the target path (default false)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Restore database', params);
      }

      const resolved = resolveMaintenance(commerce);
      if (!resolved.ok) {
        return resolved.error;
      }

      const report = await resolved.maintenance.restore(params.backupPath, params.targetPath, {
        overwrite: params.overwrite === true,
      });
      return {
        success: true,
        message: `Database restored to ${params.targetPath}`,
        targetPath: report?.targetPath ?? params.targetPath,
        schemaVersion: report?.schemaVersion,
        sizeBytes: report?.sizeBytes,
        checksumVerified: report?.checksumVerified,
        replacedExisting: report?.replacedExisting,
      };
    },
  },
  {
    name: 'export_full_data',
    description:
      'Export the full store to a versioned JSON file (distinct from export_data, which dumps a single entity type for parity testing). Covers the core commerce and finance domains; see the maintenance module docs for exactly what is and is not included.',
    inputSchema: {
      exportPath: z.string().min(1).describe('Destination path for the JSON export'),
      domains: z
        .array(z.string().min(1))
        .optional()
        .describe('Restrict the export to these domains (default: all)'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const resolved = resolveMaintenance(commerce);
      if (!resolved.ok) {
        return resolved.error;
      }

      const report = await resolved.maintenance.export(params.exportPath, {
        domains: params.domains ?? [],
      });
      return {
        success: true,
        message: `Exported ${report?.total ?? 0} records to ${params.exportPath}`,
        exportPath: params.exportPath,
        total: report?.total ?? 0,
        counts: report?.counts ?? [],
      };
    },
  },
  {
    name: 'import_full_data',
    description:
      'Import a JSON export into this store. Records are replayed through the normal create paths, so IDs are re-minted and foreign keys remapped.',
    inputSchema: {
      importPath: z.string().min(1).describe('Path to the JSON export to import'),
      domains: z
        .array(z.string().min(1))
        .optional()
        .describe('Restrict the import to these domains (default: all importable)'),
      onConflict: z
        .enum(['skip', 'fail'])
        .optional()
        .describe('What to do when a record already exists (default: skip)'),
      dryRun: z
        .boolean()
        .optional()
        .describe('Parse and validate the export without writing anything'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Import full data', params);
      }

      const resolved = resolveMaintenance(commerce);
      if (!resolved.ok) {
        return resolved.error;
      }

      const report = await resolved.maintenance.import(params.importPath, {
        domains: params.domains ?? [],
        onConflict: params.onConflict ?? 'skip',
        dryRun: params.dryRun === true,
      });
      return {
        success: true,
        message: params.dryRun
          ? `Dry run: ${params.importPath} parsed successfully`
          : `Imported ${report?.totalCreated ?? 0} records from ${params.importPath}`,
        importPath: params.importPath,
        totalCreated: report?.totalCreated ?? 0,
        created: report?.created ?? [],
        skipped: report?.skipped ?? [],
        unsupportedDomains: report?.unsupportedDomains ?? [],
      };
    },
  },
  {
    name: 'list_portable_domains',
    description: 'List the domains that data export and import can cover.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const resolved = resolveMaintenance(commerce);
      if (!resolved.ok) {
        return resolved.error;
      }

      const exportable = (await resolved.maintenance.exportableDomains()) ?? [];
      const importable = (await resolved.maintenance.importableDomains()) ?? [];
      return {
        success: true,
        exportable,
        importable,
        exportOnly: exportable.filter((domain) => !importable.includes(domain)),
      };
    },
  },
]);
