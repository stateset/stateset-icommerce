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
export const BUSINESS_PACK_MANIFEST = 'pack.yaml';
export const BUSINESS_PACK_PROFILE = 'business.yaml';

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

function readYaml(file) {
  try {
    return YAML.parse(fs.readFileSync(file, 'utf8'));
  } catch (error) {
    throw new Error(`Unable to read ${file}: ${error.message}`);
  }
}

function packSource(source) {
  const resolved = path.resolve(source);
  if (!fs.existsSync(resolved)) throw new Error(`Pack path does not exist: ${source}`);
  const stat = fs.statSync(resolved);
  if (stat.isFile()) {
    return {
      directory: path.dirname(resolved),
      profileFile: resolved,
      sourceName: path.basename(resolved).replace(/\.(ya?ml)$/i, ''),
      manifest: {},
    };
  }
  const manifestFile = path.join(resolved, BUSINESS_PACK_MANIFEST);
  const manifest = fs.existsSync(manifestFile) ? readYaml(manifestFile) : {};
  const profileFile = path.join(resolved, manifest.profile || BUSINESS_PACK_PROFILE);
  if (!fs.existsSync(profileFile)) throw new Error(`Pack has no ${BUSINESS_PACK_PROFILE}: ${resolved}`);
  return { directory: resolved, profileFile, manifest };
}

export function loadBusinessPack(source) {
  const pack = packSource(source);
  const profile = readYaml(pack.profileFile);
  const errors = validateBusinessProfile(profile);
  const name =
    pack.manifest.name ||
    pack.sourceName ||
    path.basename(pack.directory).toLowerCase().replace(/[^a-z0-9_-]+/g, '-');
  if (!NAME.test(name)) errors.push(`pack name is invalid: ${name}`);
  if (pack.manifest.version !== undefined && typeof pack.manifest.version !== 'string') {
    errors.push('pack.version must be a string');
  }
  return {
    name,
    version: pack.manifest.version || '0.1.0',
    description: pack.manifest.description || '',
    directory: pack.directory,
    profileFile: pack.profileFile,
    manifest: pack.manifest,
    profile,
    errors,
  };
}

export function installBusinessPack(source, root = process.cwd(), { force = false, preview = true } = {}) {
  const pack = loadBusinessPack(source);
  if (pack.errors.length) throw new Error(`Invalid business pack:\n- ${pack.errors.join('\n- ')}`);
  const current = loadBusinessProfile(root);
  const changes = diffBusinessProfiles(current.profile, pack.profile);
  const destination = profilePath(root);
  if (preview) return { ...pack, preview: true, destination, changes };
  const file = writeBusinessProfile(pack.profile, root, { force });
  const lockFile = path.resolve(root, '.stateset', 'packs', `${pack.name}.lock.json`);
  fs.mkdirSync(path.dirname(lockFile), { recursive: true });
  fs.writeFileSync(
    lockFile,
    `${JSON.stringify({ name: pack.name, version: pack.version, source: pack.directory, installedAt: new Date().toISOString() }, null, 2)}\n`,
    { mode: 0o600 },
  );
  return { ...pack, preview: false, destination: file, lockFile, changes };
}

export function listBusinessPacks(directory = path.resolve('profiles')) {
  if (!fs.existsSync(directory)) return [];
  return fs
    .readdirSync(directory, { withFileTypes: true })
    .map((entry) => (entry.isDirectory() ? path.join(directory, entry.name) : path.join(directory, entry.name)))
    .filter((entry) => {
      try {
        return fs.statSync(entry).isFile() ? entry.endsWith('.yaml') || entry.endsWith('.yml') : fs.existsSync(path.join(entry, BUSINESS_PACK_PROFILE));
      } catch {
        return false;
      }
    })
    .map((entry) => {
      try {
        const pack = loadBusinessPack(entry);
        return { name: pack.name, version: pack.version, description: pack.description, source: entry, valid: pack.errors.length === 0 };
      } catch (error) {
        return { name: path.basename(entry), source: entry, valid: false, errors: [error.message] };
      }
    });
}
