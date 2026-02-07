/**
 * Plugin Manifest System for StateSet iCommerce
 *
 * Defines and validates plugin manifests (stateset.plugin.json).
 * Each plugin declares its metadata, capabilities, and config schema
 * via a manifest file.
 *
 * Manifest files are discovered alongside plugin entry modules.
 */

import fs from 'fs';
import path from 'path';

// ============================================================================
// Manifest Schema
// ============================================================================

/**
 * @typedef {Object} PluginManifest
 * @property {string} id - Unique plugin identifier (e.g., 'stateset-sentiment')
 * @property {string} name - Human-readable name
 * @property {string} [version='0.0.0'] - SemVer version
 * @property {string} [description] - Plugin description
 * @property {string} [author] - Author name or email
 * @property {string} [license] - License identifier
 * @property {string} entry - Entry module path (relative to manifest)
 * @property {string} [kind='general'] - Plugin kind: 'general', 'channel', 'memory', 'provider'
 * @property {string[]} [channels] - Channels this plugin adds (for kind=channel)
 * @property {string[]} [provides] - Capabilities provided (e.g., ['commands', 'hooks', 'services'])
 * @property {boolean} [enabledByDefault=false] - Whether enabled without explicit configuration
 * @property {Object} [configSchema] - JSON Schema for plugin-specific configuration
 * @property {Object} [configDefaults] - Default values for config
 * @property {ConfigHint[]} [configHints] - UI hints for config fields
 */

/**
 * @typedef {Object} ConfigHint
 * @property {string} field - Config field path (dot notation)
 * @property {string} label - Human-readable label
 * @property {string} [help] - Help text / tooltip
 * @property {boolean} [sensitive=false] - Mask value in UI
 * @property {boolean} [advanced=false] - Hide in basic config view
 * @property {'string'|'number'|'boolean'|'select'|'multiselect'} [inputType='string']
 * @property {Array<{ label: string, value: any }>} [options] - For select/multiselect
 */

const MANIFEST_FILENAMES = ['stateset.plugin.json', 'stateset-plugin.json'];

const VALID_KINDS = new Set(['general', 'channel', 'memory', 'provider']);

const REQUIRED_FIELDS = ['id', 'name', 'entry'];

// ============================================================================
// Validation
// ============================================================================

/**
 * @typedef {Object} ManifestValidationResult
 * @property {boolean} valid
 * @property {PluginManifest} [manifest] - Parsed and normalized manifest (if valid)
 * @property {string[]} errors - Validation errors
 * @property {string[]} warnings - Non-fatal warnings
 */

/**
 * Validate a raw manifest object.
 *
 * @param {Object} raw - Raw JSON-parsed manifest
 * @param {string} [basePath] - Base directory path (for resolving relative entry)
 * @returns {ManifestValidationResult}
 */
export function validateManifest(raw, basePath) {
  const errors = [];
  const warnings = [];

  if (!raw || typeof raw !== 'object') {
    return { valid: false, errors: ['Manifest must be a JSON object'], warnings };
  }

  // Required fields
  for (const field of REQUIRED_FIELDS) {
    if (!raw[field] || typeof raw[field] !== 'string') {
      errors.push(`Missing required field: "${field}" (must be a non-empty string)`);
    }
  }

  // ID format
  if (raw.id && !/^[a-z][a-z0-9_-]*$/.test(raw.id)) {
    errors.push(`Invalid plugin ID "${raw.id}": must match /^[a-z][a-z0-9_-]*$/`);
  }

  // Kind
  if (raw.kind && !VALID_KINDS.has(raw.kind)) {
    errors.push(`Invalid kind "${raw.kind}": must be one of ${[...VALID_KINDS].join(', ')}`);
  }

  // Version format
  if (raw.version && !/^\d+\.\d+\.\d+/.test(raw.version)) {
    warnings.push(`Version "${raw.version}" does not follow SemVer format`);
  }

  // Entry file resolution (warn if not found, but don't fail validation)
  if (raw.entry && basePath) {
    const entryPath = path.resolve(basePath, raw.entry);
    if (!fs.existsSync(entryPath)) {
      warnings.push(`Entry file not found: ${entryPath}`);
    }
  }

  // Config schema validation
  if (raw.configSchema && typeof raw.configSchema !== 'object') {
    errors.push('configSchema must be a JSON Schema object');
  }

  // Channels array
  if (raw.channels && !Array.isArray(raw.channels)) {
    errors.push('channels must be an array of strings');
  }

  // Provides array
  if (raw.provides && !Array.isArray(raw.provides)) {
    errors.push('provides must be an array of strings');
  }

  if (errors.length > 0) {
    return { valid: false, errors, warnings };
  }

  // Normalize
  const manifest = {
    id: raw.id,
    name: raw.name,
    version: raw.version || '0.0.0',
    description: raw.description || '',
    author: raw.author || '',
    license: raw.license || '',
    entry: raw.entry,
    kind: raw.kind || 'general',
    channels: raw.channels || [],
    provides: raw.provides || [],
    enabledByDefault: raw.enabledByDefault === true,
    configSchema: raw.configSchema || null,
    configDefaults: raw.configDefaults || {},
    configHints: raw.configHints || [],
  };

  return { valid: true, manifest, errors, warnings };
}

// ============================================================================
// File I/O
// ============================================================================

/**
 * Read and parse a plugin manifest from a directory.
 *
 * @param {string} dirPath - Directory to search for manifest
 * @returns {{ found: boolean, manifest?: PluginManifest, path?: string, errors?: string[], warnings?: string[] }}
 */
export function readManifest(dirPath) {
  for (const filename of MANIFEST_FILENAMES) {
    const manifestPath = path.join(dirPath, filename);

    if (!fs.existsSync(manifestPath)) continue;

    try {
      const raw = JSON.parse(fs.readFileSync(manifestPath, 'utf-8'));
      const result = validateManifest(raw, dirPath);

      if (result.valid) {
        return {
          found: true,
          manifest: result.manifest,
          path: manifestPath,
          warnings: result.warnings,
        };
      }

      return {
        found: true,
        path: manifestPath,
        errors: result.errors,
        warnings: result.warnings,
      };
    } catch (err) {
      return {
        found: true,
        path: manifestPath,
        errors: [`Failed to parse manifest: ${err.message}`],
        warnings: [],
      };
    }
  }

  return { found: false };
}

/**
 * Validate a plugin's config against its manifest configSchema.
 *
 * Basic JSON Schema validation (type, required, enum).
 *
 * @param {Object} config - Plugin configuration object
 * @param {Object} schema - JSON Schema from manifest
 * @returns {{ valid: boolean, errors: string[] }}
 */
export function validateConfig(config, schema) {
  if (!schema || typeof schema !== 'object') {
    return { valid: true, errors: [] };
  }

  const errors = [];

  // Validate required fields
  if (schema.required && Array.isArray(schema.required)) {
    for (const field of schema.required) {
      if (config[field] === undefined || config[field] === null) {
        errors.push(`Missing required config field: "${field}"`);
      }
    }
  }

  // Validate property types
  if (schema.properties && typeof schema.properties === 'object') {
    for (const [field, fieldSchema] of Object.entries(schema.properties)) {
      const value = config[field];
      if (value === undefined) continue;

      if (fieldSchema.type) {
        const actualType = Array.isArray(value) ? 'array' : typeof value;
        if (actualType !== fieldSchema.type) {
          errors.push(`Config field "${field}": expected ${fieldSchema.type}, got ${actualType}`);
        }
      }

      if (fieldSchema.enum && Array.isArray(fieldSchema.enum)) {
        if (!fieldSchema.enum.includes(value)) {
          errors.push(`Config field "${field}": must be one of ${fieldSchema.enum.join(', ')}`);
        }
      }

      if (
        fieldSchema.minLength &&
        typeof value === 'string' &&
        value.length < fieldSchema.minLength
      ) {
        errors.push(`Config field "${field}": minimum length ${fieldSchema.minLength}`);
      }

      if (
        fieldSchema.minimum !== undefined &&
        typeof value === 'number' &&
        value < fieldSchema.minimum
      ) {
        errors.push(`Config field "${field}": minimum value ${fieldSchema.minimum}`);
      }

      if (
        fieldSchema.maximum !== undefined &&
        typeof value === 'number' &&
        value > fieldSchema.maximum
      ) {
        errors.push(`Config field "${field}": maximum value ${fieldSchema.maximum}`);
      }
    }
  }

  return { valid: errors.length === 0, errors };
}

/**
 * Apply default values from a manifest to a config object.
 *
 * @param {Object} config - User-provided config
 * @param {Object} defaults - Manifest configDefaults
 * @returns {Object} - Merged config with defaults
 */
export function applyConfigDefaults(config, defaults) {
  if (!defaults || typeof defaults !== 'object') return { ...config };

  const result = { ...defaults };

  for (const [key, value] of Object.entries(config)) {
    if (value !== undefined && value !== null) {
      result[key] = value;
    }
  }

  return result;
}
