/**
 * Portable, declarative business profile support.
 *
 * A profile describes the business layer around the commerce kernel. It is
 * deliberately data-only: applying a profile can configure defaults and
 * declarations, but cannot execute arbitrary JavaScript or SQL.
 */
import fs from 'node:fs';
import path from 'node:path';
import YAML from 'yaml';

export const BUSINESS_PROFILE_VERSION = 1;
export const BUSINESS_PROFILE_FILE = path.join('.stateset', 'business.yaml');

export const DEFAULT_BUSINESS_PROFILE = {
  schemaVersion: BUSINESS_PROFILE_VERSION,
  business: {
    name: 'My business',
    currency: 'USD',
    timezone: 'UTC',
  },
  terminology: {},
  modules: {
    orders: true,
    inventory: true,
    payments: true,
    returns: true,
  },
  policies: [],
  workflows: [],
  views: [],
  automations: [],
  integrations: [],
};

const NAME = /^[a-z][a-z0-9_-]{0,62}$/;
const CURRENCY = /^[A-Z]{3}$/;

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function merge(base, value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return value;
  const result = { ...base };
  for (const [key, entry] of Object.entries(value)) {
    result[key] =
      entry && typeof entry === 'object' && !Array.isArray(entry)
        ? merge(result[key] && typeof result[key] === 'object' ? result[key] : {}, entry)
        : entry;
  }
  return result;
}

function profilePath(root = process.cwd()) {
  return path.resolve(root, BUSINESS_PROFILE_FILE);
}

export function validateBusinessProfile(profile) {
  const errors = [];
  if (!profile || typeof profile !== 'object' || Array.isArray(profile)) {
    return ['profile must be a mapping'];
  }
  if (profile.schemaVersion !== BUSINESS_PROFILE_VERSION) {
    errors.push(`schemaVersion must be ${BUSINESS_PROFILE_VERSION}`);
  }
  if (!profile.business || typeof profile.business !== 'object') {
    errors.push('business must be a mapping');
  } else {
    if (typeof profile.business.name !== 'string' || profile.business.name.trim() === '') {
      errors.push('business.name must be a non-empty string');
    }
    if (!CURRENCY.test(String(profile.business.currency || ''))) {
      errors.push('business.currency must be a three-letter uppercase code');
    }
    if (typeof profile.business.timezone !== 'string' || profile.business.timezone.trim() === '') {
      errors.push('business.timezone must be a non-empty string');
    }
  }
  if (!profile.modules || typeof profile.modules !== 'object' || Array.isArray(profile.modules)) {
    errors.push('modules must be a mapping');
  } else {
    for (const [key, enabled] of Object.entries(profile.modules)) {
      if (!NAME.test(key)) errors.push(`module name is invalid: ${key}`);
      if (typeof enabled !== 'boolean') errors.push(`module ${key} must be boolean`);
    }
  }
  for (const key of ['policies', 'workflows', 'views', 'automations', 'integrations']) {
    if (!Array.isArray(profile[key])) errors.push(`${key} must be a list`);
    else {
      for (const item of profile[key]) {
        if (!item || typeof item !== 'object' || Array.isArray(item)) {
          errors.push(`${key} entries must be mappings`);
        } else if (typeof item.name !== 'string' || !NAME.test(item.name)) {
          errors.push(`${key} entries need a lowercase name`);
        }
      }
    }
  }
  return errors;
}

export function loadBusinessProfile(root = process.cwd()) {
  const file = profilePath(root);
  if (!fs.existsSync(file)) return { profile: clone(DEFAULT_BUSINESS_PROFILE), file, exists: false };
  let parsed;
  try {
    parsed = YAML.parse(fs.readFileSync(file, 'utf8'));
  } catch (error) {
    return { profile: null, file, exists: true, errors: [`invalid YAML: ${error.message}`] };
  }
  const profile = merge(clone(DEFAULT_BUSINESS_PROFILE), parsed);
  return { profile, file, exists: true, errors: validateBusinessProfile(profile) };
}

export function writeBusinessProfile(profile, root = process.cwd(), { force = false } = {}) {
  const errors = validateBusinessProfile(profile);
  if (errors.length) throw new Error(`Invalid business profile:\n- ${errors.join('\n- ')}`);
  const file = profilePath(root);
  if (fs.existsSync(file) && !force) throw new Error(`Profile already exists: ${file} (use --force)`);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, YAML.stringify(profile), { mode: 0o600 });
  return file;
}

export function initBusinessProfile(root = process.cwd(), options = {}) {
  const profile = merge(clone(DEFAULT_BUSINESS_PROFILE), options.profile || {});
  return writeBusinessProfile(profile, root, options);
}

function flatten(value, prefix = '', output = new Map()) {
  if (value && typeof value === 'object' && !Array.isArray(value)) {
    for (const key of Object.keys(value).sort()) flatten(value[key], prefix ? `${prefix}.${key}` : key, output);
  } else {
    output.set(prefix, JSON.stringify(value));
  }
  return output;
}

export function diffBusinessProfiles(before, after) {
  const left = flatten(before);
  const right = flatten(after);
  const keys = [...new Set([...left.keys(), ...right.keys()])].sort();
  return keys
    .filter((key) => left.get(key) !== right.get(key))
    .map((key) => ({ path: key, before: left.get(key), after: right.get(key) }));
}

export function businessProfileDoctor(root = process.cwd()) {
  const loaded = loadBusinessProfile(root);
  return {
    ready: loaded.exists && loaded.errors?.length === 0,
    file: loaded.file,
    exists: loaded.exists,
    errors: loaded.errors || [],
    profile: loaded.profile,
  };
}
