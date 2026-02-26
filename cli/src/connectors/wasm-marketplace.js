import fs from 'node:fs';
import fsp from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { createHash, createHmac, randomUUID } from 'node:crypto';
import { WASI } from 'node:wasi';

export const CONNECTOR_SCHEMA_VERSION = 'wasm-connector/v1';
export const CONNECTOR_CATALOG_SCHEMA_VERSION = 'wasm-catalog/v1';
export const CONNECTOR_RUNTIME_KINDS = ['native-export', 'wasi-command'];

const CATALOG_FILE = 'catalog.json';
const INSTALLED_DIR = 'installed';
const MANIFEST_FILE = 'manifest.json';
const MODULE_FILE = 'module.wasm';
const CONNECTOR_ID_PATTERN = /^[a-z0-9][a-z0-9._-]{1,63}$/;
const CONNECTOR_VERSION_PATTERN = /^[0-9A-Za-z][0-9A-Za-z._+-]{0,63}$/;
const CONNECTOR_ACTION_PATTERN = /^[a-zA-Z][a-zA-Z0-9_:-]{1,63}$/;
const CONNECTOR_ATTESTATION_SCHEMA_VERSION = 'wasm-connector-attestation/v1';
const CONNECTOR_ATTESTATION_ALGO_UNSIGNED = 'deterministic-sha256';
const CONNECTOR_ATTESTATION_ALGO_HMAC = 'hmac-sha256';
const CONNECTOR_CERTIFICATION_SCHEMA_VERSION = 'wasm-connector-certification/v1';
const CONNECTOR_SAFETY_POLICY_VERSION = '2026-03-01';
const CONNECTOR_CERTIFICATION_STATUSES = new Set(['candidate', 'certified', 'revoked']);
const STRICT_VERIFY_TRUE_VALUES = new Set(['1', 'true', 'yes', 'on']);

const wasmModuleCache = new Map();

function sha256Buffer(buffer) {
  return createHash('sha256').update(buffer).digest('hex');
}

async function sha256File(filePath) {
  const bytes = await fsp.readFile(filePath);
  return sha256Buffer(bytes);
}

function normalizeStableValue(value) {
  if (value === null) return null;
  if (value === undefined) return undefined;
  if (typeof value === 'bigint') return value.toString();
  if (Array.isArray(value)) {
    return value.map((item) => {
      const normalized = normalizeStableValue(item);
      return normalized === undefined ? null : normalized;
    });
  }
  if (typeof value === 'object') {
    const normalizedObject = {};
    for (const key of Object.keys(value).sort((left, right) => left.localeCompare(right))) {
      const normalized = normalizeStableValue(value[key]);
      if (normalized !== undefined) {
        normalizedObject[key] = normalized;
      }
    }
    return normalizedObject;
  }
  if (typeof value === 'number') {
    return Number.isFinite(value) ? value : null;
  }
  if (typeof value === 'function' || typeof value === 'symbol') {
    return undefined;
  }
  return value;
}

function stableStringify(value) {
  return JSON.stringify(normalizeStableValue(value));
}

function resolveConnectorSigningKey(explicitKey = null) {
  const candidate = explicitKey ?? process.env.STATESET_CONNECTOR_SIGNING_KEY;
  if (candidate === null || candidate === undefined) return null;
  const normalized = String(candidate).trim();
  return normalized.length > 0 ? normalized : null;
}

function isStrictConnectorVerificationEnabled(verifyStrict = null) {
  if (verifyStrict === true || verifyStrict === false) return verifyStrict;
  const normalized = String(process.env.STATESET_CONNECTOR_VERIFY_STRICT || '')
    .trim()
    .toLowerCase();
  return STRICT_VERIFY_TRUE_VALUES.has(normalized);
}

function isConnectorCertificationRequired(requireCertified = null) {
  if (requireCertified === true || requireCertified === false) return requireCertified;
  const normalized = String(process.env.STATESET_CONNECTOR_REQUIRE_CERTIFIED || '')
    .trim()
    .toLowerCase();
  return STRICT_VERIFY_TRUE_VALUES.has(normalized);
}

function clampSafetyScore(value) {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(100, Math.round(value)));
}

function resolveConnectorMinSafetyScore(minSafetyScore = null) {
  const candidate =
    minSafetyScore !== null && minSafetyScore !== undefined
      ? minSafetyScore
      : process.env.STATESET_CONNECTOR_MIN_SAFETY_SCORE;
  if (candidate === null || candidate === undefined || String(candidate).trim() === '') {
    return null;
  }
  const parsed = Number(candidate);
  if (!Number.isFinite(parsed)) {
    throw new Error(
      `Invalid connector min safety score "${candidate}". Expected a number between 0 and 100.`,
    );
  }
  return clampSafetyScore(parsed);
}

function normalizeConnectorId(value) {
  const normalized = String(value || '')
    .trim()
    .toLowerCase();
  if (!CONNECTOR_ID_PATTERN.test(normalized)) {
    throw new Error(
      `Invalid connectorId "${value}". Use 2-64 chars: lowercase letters, digits, ".", "_" or "-".`,
    );
  }
  return normalized;
}

function normalizeVersion(value) {
  const normalized = String(value || '').trim();
  if (!CONNECTOR_VERSION_PATTERN.test(normalized)) {
    throw new Error(
      `Invalid version "${value}". Use 1-64 chars: letters, digits, ".", "_", "+", "-".`,
    );
  }
  return normalized;
}

function normalizeActionName(value) {
  const normalized = String(value || '').trim();
  if (!CONNECTOR_ACTION_PATTERN.test(normalized)) {
    throw new Error(`Invalid action name "${value}". Use 2-64 chars and start with a letter.`);
  }
  return normalized;
}

function normalizeRuntimeKind(value) {
  const resolved = String(value || 'native-export')
    .trim()
    .toLowerCase();
  if (!CONNECTOR_RUNTIME_KINDS.includes(resolved)) {
    throw new Error(
      `Unsupported runtime kind "${value}". Expected one of: ${CONNECTOR_RUNTIME_KINDS.join(', ')}.`,
    );
  }
  return resolved;
}

function semverLikeCompare(a, b) {
  const splitA = String(a || '')
    .split('.')
    .map((part) => {
      const parsed = Number.parseInt(part, 10);
      return Number.isFinite(parsed) ? parsed : part;
    });
  const splitB = String(b || '')
    .split('.')
    .map((part) => {
      const parsed = Number.parseInt(part, 10);
      return Number.isFinite(parsed) ? parsed : part;
    });

  const len = Math.max(splitA.length, splitB.length);
  for (let i = 0; i < len; i += 1) {
    const left = splitA[i] ?? 0;
    const right = splitB[i] ?? 0;
    if (typeof left === 'number' && typeof right === 'number') {
      if (left !== right) return left - right;
      continue;
    }
    const leftStr = String(left);
    const rightStr = String(right);
    if (leftStr !== rightStr) {
      return leftStr.localeCompare(rightStr);
    }
  }
  return 0;
}

async function pathExists(targetPath) {
  try {
    await fsp.access(targetPath);
    return true;
  } catch {
    return false;
  }
}

async function ensureDir(targetPath) {
  await fsp.mkdir(targetPath, { recursive: true });
}

async function readJson(targetPath, fallback) {
  if (!(await pathExists(targetPath))) return fallback;
  const raw = await fsp.readFile(targetPath, 'utf8');
  if (!raw.trim()) return fallback;
  try {
    return JSON.parse(raw);
  } catch (error) {
    throw new Error(`Failed to parse JSON at ${targetPath}: ${error.message}`);
  }
}

async function writeJson(targetPath, value) {
  await ensureDir(path.dirname(targetPath));
  const tempPath = `${targetPath}.tmp`;
  await fsp.writeFile(tempPath, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
  await fsp.rename(tempPath, targetPath);
}

function getCatalogFile(connectorHome) {
  return path.join(connectorHome, CATALOG_FILE);
}

function getInstalledRoot(connectorHome) {
  return path.join(connectorHome, INSTALLED_DIR);
}

function getInstalledVersionDir(connectorHome, connectorId, version) {
  return path.join(getInstalledRoot(connectorHome), connectorId, version);
}

function getManifestPath(connectorHome, connectorId, version) {
  return path.join(getInstalledVersionDir(connectorHome, connectorId, version), MANIFEST_FILE);
}

function getModulePath(connectorHome, connectorId, version) {
  return path.join(getInstalledVersionDir(connectorHome, connectorId, version), MODULE_FILE);
}

async function loadCatalog(connectorHome) {
  const catalogFile = getCatalogFile(connectorHome);
  const initial = {
    schemaVersion: CONNECTOR_CATALOG_SCHEMA_VERSION,
    updatedAt: new Date(0).toISOString(),
    connectors: [],
  };
  const catalog = await readJson(catalogFile, initial);
  if (!Array.isArray(catalog.connectors)) {
    catalog.connectors = [];
  }
  return catalog;
}

async function saveCatalog(connectorHome, catalog) {
  const catalogFile = getCatalogFile(connectorHome);
  await writeJson(catalogFile, {
    schemaVersion: CONNECTOR_CATALOG_SCHEMA_VERSION,
    updatedAt: new Date().toISOString(),
    connectors: catalog.connectors || [],
  });
}

async function compileWasmModule(wasmPath) {
  const bytes = await fsp.readFile(wasmPath);
  const moduleHash = sha256Buffer(bytes);
  if (wasmModuleCache.has(moduleHash)) {
    return {
      moduleHash,
      module: wasmModuleCache.get(moduleHash),
    };
  }
  const module = await WebAssembly.compile(bytes);
  wasmModuleCache.set(moduleHash, module);
  return { moduleHash, module };
}

async function resolveNativeExportActions({ wasmPath, declaredActions }) {
  const { module } = await compileWasmModule(wasmPath);
  const exports = WebAssembly.Module.exports(module);
  const functionExportNames = exports
    .filter((entry) => entry.kind === 'function')
    .map((entry) => entry.name);

  if (declaredActions.length === 0) {
    return functionExportNames.map((name) => ({
      name,
      exportName: name,
      description: `Invoke exported WASM function "${name}"`,
      args: [],
      commandArgs: [],
      timeoutMs: null,
      inputSchema: null,
    }));
  }

  const normalized = declaredActions.map((action) => {
    const exportName = String(action.exportName || action.name || '').trim();
    if (!functionExportNames.includes(exportName)) {
      throw new Error(
        `Connector action "${action.name}" references missing export "${exportName}". Available exports: ${functionExportNames.join(', ') || '(none)'}.`,
      );
    }
    return {
      ...action,
      exportName,
    };
  });

  return normalized;
}

function normalizeConnectorActions(actions = []) {
  const seen = new Set();
  return (Array.isArray(actions) ? actions : []).map((action, index) => {
    const name = normalizeActionName(action?.name || `action_${index + 1}`);
    if (seen.has(name)) {
      throw new Error(`Duplicate connector action name "${name}".`);
    }
    seen.add(name);
    return {
      name,
      exportName: action?.exportName ? String(action.exportName).trim() : null,
      description: action?.description ? String(action.description) : null,
      args: Array.isArray(action?.args) ? action.args.map((arg) => String(arg)) : [],
      commandArgs: Array.isArray(action?.commandArgs)
        ? action.commandArgs.map((arg) => String(arg))
        : [],
      timeoutMs:
        action?.timeoutMs === undefined || action?.timeoutMs === null
          ? null
          : Number(action.timeoutMs),
      inputSchema:
        action?.inputSchema && typeof action.inputSchema === 'object' ? action.inputSchema : null,
    };
  });
}

function buildConnectorAttestationPayload(entry) {
  return {
    schemaVersion: CONNECTOR_SCHEMA_VERSION,
    id: entry?.id ? String(entry.id) : null,
    version: entry?.version ? String(entry.version) : null,
    name: entry?.name ? String(entry.name) : null,
    description: entry?.description ? String(entry.description) : null,
    runtimeKind: entry?.runtime?.kind ? String(entry.runtime.kind) : null,
    actions: Array.isArray(entry?.actions)
      ? entry.actions.map((action) => ({
          name: action?.name ? String(action.name) : null,
          exportName: action?.exportName ? String(action.exportName) : null,
          args: Array.isArray(action?.args) ? action.args.map((arg) => String(arg)) : [],
          commandArgs: Array.isArray(action?.commandArgs)
            ? action.commandArgs.map((arg) => String(arg))
            : [],
          timeoutMs:
            action?.timeoutMs === undefined || action?.timeoutMs === null
              ? null
              : Number(action.timeoutMs),
          inputSchema:
            action?.inputSchema && typeof action.inputSchema === 'object'
              ? action.inputSchema
              : null,
        }))
      : [],
    tags: Array.isArray(entry?.tags) ? entry.tags.map((tag) => String(tag)) : [],
    publisher: entry?.publisher ? String(entry.publisher) : null,
    wasmSha256: entry?.wasmSha256 ? String(entry.wasmSha256) : null,
    sourceType: entry?.source?.type ? String(entry.source.type) : null,
  };
}

function buildConnectorAttestation({
  entry,
  signingKey = null,
  keyId = null,
  signedBy = null,
} = {}) {
  const payload = buildConnectorAttestationPayload(entry);
  const payloadText = stableStringify(payload);
  const payloadHash = sha256Buffer(Buffer.from(payloadText));
  const key = resolveConnectorSigningKey(signingKey);
  const algorithm = key ? CONNECTOR_ATTESTATION_ALGO_HMAC : CONNECTOR_ATTESTATION_ALGO_UNSIGNED;
  const signature =
    algorithm === CONNECTOR_ATTESTATION_ALGO_HMAC
      ? createHmac('sha256', key).update(payloadText).digest('hex')
      : payloadHash;

  return {
    schemaVersion: CONNECTOR_ATTESTATION_SCHEMA_VERSION,
    algorithm,
    payloadHash,
    signature,
    keyId: keyId ? String(keyId) : null,
    signedBy: signedBy ? String(signedBy) : key ? 'local-signing-key' : 'unsigned',
    signedAt: new Date().toISOString(),
  };
}

function verifyConnectorEntryAttestation({ entry, signingKey = null } = {}) {
  const attestation = entry?.attestation;
  if (!attestation || typeof attestation !== 'object') {
    return {
      valid: false,
      reason: 'missing_attestation',
      hasAttestation: false,
      algorithm: null,
      keyRequired: false,
      keyAvailable: Boolean(resolveConnectorSigningKey(signingKey)),
      checks: {
        payloadHashMatches: false,
        signatureMatches: false,
      },
    };
  }

  const payload = buildConnectorAttestationPayload(entry);
  const payloadText = stableStringify(payload);
  const payloadHash = sha256Buffer(Buffer.from(payloadText));
  const algorithm = String(attestation.algorithm || '').trim();
  const signature = String(attestation.signature || '').trim();
  const key = resolveConnectorSigningKey(signingKey);

  let expectedSignature = null;
  let reason = null;
  let keyRequired = false;

  if (algorithm === CONNECTOR_ATTESTATION_ALGO_UNSIGNED) {
    expectedSignature = payloadHash;
  } else if (algorithm === CONNECTOR_ATTESTATION_ALGO_HMAC) {
    keyRequired = true;
    if (!key) {
      reason = 'missing_signing_key';
    } else {
      expectedSignature = createHmac('sha256', key).update(payloadText).digest('hex');
    }
  } else {
    reason = 'unsupported_algorithm';
  }

  const payloadHashMatches = String(attestation.payloadHash || '') === payloadHash;
  const signatureMatches =
    expectedSignature !== null && signature.length > 0 && signature === expectedSignature;
  const valid = reason === null && payloadHashMatches && signatureMatches;

  if (!valid && reason === null) {
    reason = !payloadHashMatches ? 'payload_hash_mismatch' : 'signature_mismatch';
  }

  return {
    valid,
    reason,
    hasAttestation: true,
    algorithm: algorithm || null,
    keyRequired,
    keyAvailable: Boolean(key),
    checks: {
      payloadHashMatches,
      signatureMatches,
    },
  };
}

function resolveCatalogConnectorEntry(catalog, connectorId, version = null) {
  const id = normalizeConnectorId(connectorId);
  const candidates = (catalog.connectors || [])
    .map((entry, index) => ({ entry, index }))
    .filter((item) => item.entry?.id === id);

  if (candidates.length === 0) {
    throw new Error(`Connector "${id}" was not found in the marketplace catalog.`);
  }

  if (version !== null && version !== undefined) {
    const resolvedVersion = normalizeVersion(version);
    const exact = candidates.find((candidate) => candidate.entry?.version === resolvedVersion);
    if (!exact) {
      throw new Error(
        `Connector "${id}@${resolvedVersion}" was not found in the marketplace catalog.`,
      );
    }
    return exact;
  }

  return candidates
    .slice()
    .sort((left, right) => semverLikeCompare(right.entry.version, left.entry.version))[0];
}

function assertConnectorAttestationPolicy({
  verification,
  strict,
  connectorId,
  version,
  operation,
}) {
  if (verification.valid) return;
  if (!strict) return;
  const reason = verification.reason || 'invalid_attestation';
  throw new Error(
    `Connector attestation verification failed for "${connectorId}@${version}" during ${operation}: ${reason}.`,
  );
}

function normalizeCertificationStatus(value = 'certified') {
  const status = String(value || '')
    .trim()
    .toLowerCase();
  if (!CONNECTOR_CERTIFICATION_STATUSES.has(status)) {
    throw new Error(
      `Unsupported certification status "${value}". Expected one of: ${Array.from(CONNECTOR_CERTIFICATION_STATUSES).join(', ')}.`,
    );
  }
  return status;
}

function getConnectorCertificationState(entry) {
  const certification =
    entry?.certification && typeof entry.certification === 'object' ? entry.certification : null;
  const status = certification?.status ? String(certification.status).toLowerCase() : 'none';
  const safetyScore =
    certification?.safetyScore === null || certification?.safetyScore === undefined
      ? null
      : Number(certification.safetyScore);
  const normalizedSafetyScore = Number.isFinite(safetyScore) ? clampSafetyScore(safetyScore) : null;

  return {
    hasCertification: Boolean(certification),
    status,
    certified: status === 'certified',
    safetyScore: normalizedSafetyScore,
    level: certification?.level ? String(certification.level) : null,
    issuedAt: certification?.issuedAt ? String(certification.issuedAt) : null,
    assessor: certification?.assessor ? String(certification.assessor) : null,
  };
}

function assessConnectorEntrySafety({
  entry,
  attestationVerification = null,
  signingKey = null,
} = {}) {
  const runtimeKind = normalizeRuntimeKind(entry?.runtime?.kind || 'native-export');
  const actions = Array.isArray(entry?.actions) ? entry.actions : [];
  const tags = Array.isArray(entry?.tags) ? entry.tags : [];
  const verification =
    attestationVerification || verifyConnectorEntryAttestation({ entry, signingKey });
  let score = 60;
  const riskFlags = [];

  if (runtimeKind === 'native-export') {
    score += 10;
  } else {
    score -= 5;
    riskFlags.push('wasi_command_runtime');
  }

  if (actions.length === 0) {
    score -= 15;
    riskFlags.push('no_actions');
  } else if (actions.length > 10) {
    score -= Math.min(20, (actions.length - 10) * 2);
    riskFlags.push('large_action_surface');
  }

  const actionsWithUnboundedArgs = actions.filter(
    (action) => !Array.isArray(action?.args) || action.args.length === 0,
  ).length;
  if (actionsWithUnboundedArgs > 0) {
    score -= Math.min(12, actionsWithUnboundedArgs * 2);
    riskFlags.push('unbounded_action_arguments');
  }

  const hasLongTimeout = actions.some(
    (action) => Number.isFinite(Number(action?.timeoutMs)) && Number(action.timeoutMs) > 120000,
  );
  if (hasLongTimeout) {
    score -= 5;
    riskFlags.push('long_running_actions');
  }

  if (entry?.publisher) score += 5;
  else riskFlags.push('missing_publisher');

  if (tags.length > 0) score += 3;
  if (String(entry?.description || '').trim().length >= 20) score += 2;

  if (entry?.wasmSha256 && String(entry.wasmSha256).length === 64) {
    score += 5;
  } else {
    score -= 10;
    riskFlags.push('missing_wasm_integrity_hash');
  }

  if (verification.valid) {
    score += verification.algorithm === CONNECTOR_ATTESTATION_ALGO_HMAC ? 15 : 8;
  } else {
    score -= 20;
    riskFlags.push(`attestation_${verification.reason || 'invalid'}`);
  }

  const normalizedScore = clampSafetyScore(score);
  const tier = normalizedScore >= 85 ? 'trusted' : normalizedScore >= 70 ? 'moderate' : 'high';
  const recommendation =
    normalizedScore >= 80 ? 'certify' : normalizedScore >= 60 ? 'review' : 'block';

  return {
    schemaVersion: 'wasm-connector-safety/v1',
    policyVersion: CONNECTOR_SAFETY_POLICY_VERSION,
    score: normalizedScore,
    tier,
    recommendation,
    risks: Array.from(new Set(riskFlags)),
    evidence: {
      runtimeKind,
      actionCount: actions.length,
      actionsWithUnboundedArgs,
      hasLongTimeout,
      publisherPresent: Boolean(entry?.publisher),
      tagCount: tags.length,
      descriptionPresent: String(entry?.description || '').trim().length > 0,
      wasmHashPresent: Boolean(entry?.wasmSha256),
      attestationValid: verification.valid,
      attestationAlgorithm: verification.algorithm,
    },
    assessedAt: new Date().toISOString(),
  };
}

function assertConnectorCertificationPolicy({
  entry,
  safetyAssessment,
  requireCertified,
  minSafetyScore,
  connectorId,
  version,
  operation,
}) {
  const certificationState = getConnectorCertificationState(entry);
  const effectiveScore = certificationState.safetyScore ?? safetyAssessment?.score ?? null;

  if (requireCertified && !certificationState.certified) {
    throw new Error(
      `Connector certification policy failed for "${connectorId}@${version}" during ${operation}: certification status is "${certificationState.status}".`,
    );
  }

  if (minSafetyScore !== null && minSafetyScore !== undefined) {
    if (!Number.isFinite(effectiveScore)) {
      throw new Error(
        `Connector certification policy failed for "${connectorId}@${version}" during ${operation}: missing safety score.`,
      );
    }
    if (effectiveScore < minSafetyScore) {
      throw new Error(
        `Connector certification policy failed for "${connectorId}@${version}" during ${operation}: safety score ${effectiveScore} is below required minimum ${minSafetyScore}.`,
      );
    }
  }
}

async function readInstalledManifest(connectorHome, connectorId, version) {
  const manifestPath = getManifestPath(connectorHome, connectorId, version);
  const manifest = await readJson(manifestPath, null);
  if (!manifest) return null;
  return {
    ...manifest,
    connectorId,
    version,
    manifestPath,
    modulePath: getModulePath(connectorHome, connectorId, version),
  };
}

function pickLatestVersion(versions) {
  if (!Array.isArray(versions) || versions.length === 0) return null;
  return versions.slice().sort((left, right) => semverLikeCompare(right, left))[0];
}

async function resolveInstalledManifest(connectorHome, connectorId, version = null) {
  const normalizedConnectorId = normalizeConnectorId(connectorId);
  if (version) {
    const normalizedVersion = normalizeVersion(version);
    const manifest = await readInstalledManifest(
      connectorHome,
      normalizedConnectorId,
      normalizedVersion,
    );
    if (!manifest) {
      throw new Error(
        `Connector "${normalizedConnectorId}@${normalizedVersion}" is not installed.`,
      );
    }
    return manifest;
  }

  const connectorRoot = path.join(getInstalledRoot(connectorHome), normalizedConnectorId);
  if (!(await pathExists(connectorRoot))) {
    throw new Error(`Connector "${normalizedConnectorId}" is not installed.`);
  }
  const versions = (
    await fsp.readdir(connectorRoot, {
      withFileTypes: true,
    })
  )
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name);
  const latestVersion = pickLatestVersion(versions);
  if (!latestVersion) {
    throw new Error(`Connector "${normalizedConnectorId}" has no installed versions.`);
  }
  const manifest = await readInstalledManifest(connectorHome, normalizedConnectorId, latestVersion);
  if (!manifest) {
    throw new Error(
      `Connector "${normalizedConnectorId}@${latestVersion}" is missing its manifest.`,
    );
  }
  return manifest;
}

function normalizeWasmScalar(value) {
  if (typeof value === 'bigint') {
    const asNumber = Number(value);
    return Number.isSafeInteger(asNumber) ? asNumber : value.toString();
  }
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) {
      throw new Error(`WASM function returned non-finite number "${value}".`);
    }
    return value;
  }
  if (typeof value === 'boolean') return value;
  if (value === null || value === undefined) return null;
  return String(value);
}

function coerceNativeArg(rawValue, argName) {
  if (typeof rawValue === 'number') {
    if (!Number.isFinite(rawValue)) {
      throw new Error(`Argument "${argName}" must be a finite number.`);
    }
    return rawValue;
  }
  if (typeof rawValue === 'boolean') return rawValue ? 1 : 0;
  if (typeof rawValue === 'string') {
    const parsed = Number(rawValue);
    if (!Number.isFinite(parsed)) {
      throw new Error(`Argument "${argName}" must be numeric for native-export runtime.`);
    }
    return parsed;
  }
  throw new Error(`Argument "${argName}" must be number, boolean, or numeric string.`);
}

async function executeNativeExportAction({ modulePath, action, params = {}, timeoutMs = null }) {
  const startedAt = Date.now();
  const { module } = await compileWasmModule(modulePath);
  const instance = await WebAssembly.instantiate(module, {});
  const exportName = action.exportName || action.name;
  const fn = instance?.exports?.[exportName];
  if (typeof fn !== 'function') {
    throw new Error(`WASM export "${exportName}" is not callable.`);
  }

  const argNames =
    Array.isArray(action.args) && action.args.length > 0
      ? action.args
      : Object.keys(params || {}).sort((left, right) => left.localeCompare(right));
  const args = argNames.map((argName) => coerceNativeArg(params?.[argName], argName));
  const value = normalizeWasmScalar(fn(...args));
  const elapsedMs = Date.now() - startedAt;
  if (Number.isFinite(timeoutMs) && timeoutMs > 0 && elapsedMs > timeoutMs) {
    throw new Error(`Connector action exceeded timeout (${elapsedMs}ms > ${timeoutMs}ms).`);
  }

  return {
    output: { value },
    execution: {
      elapsedMs,
      args: argNames,
    },
  };
}

function getWasiImportObject(wasi) {
  if (typeof wasi.getImportObject === 'function') {
    return wasi.getImportObject();
  }
  return {
    wasi_snapshot_preview1: wasi.wasiImport,
  };
}

function createWasiInstance(options) {
  try {
    return new WASI({ version: 'preview1', ...options });
  } catch {
    return new WASI(options);
  }
}

async function executeWasiCommandAction({
  modulePath,
  connectorId,
  connectorVersion,
  action,
  params = {},
  context = {},
  timeoutMs = null,
  workingDirectory,
}) {
  const requestEnvelope = {
    schemaVersion: 'wasm-connector-exec/v1',
    requestId: randomUUID(),
    connectorId,
    connectorVersion,
    action: action.name,
    params,
    context,
    occurredAt: new Date().toISOString(),
  };

  const tempRoot = await fsp.mkdtemp(path.join(os.tmpdir(), 'stateset-wasi-connector-'));
  const stdinPath = path.join(tempRoot, 'stdin.json');
  const stdoutPath = path.join(tempRoot, 'stdout.json');
  const stderrPath = path.join(tempRoot, 'stderr.log');
  let stdinFd = null;
  let stdoutFd = null;
  let stderrFd = null;
  const startedAt = Date.now();

  try {
    await fsp.writeFile(stdinPath, `${JSON.stringify(requestEnvelope)}\n`, 'utf8');
    await fsp.writeFile(stdoutPath, '', 'utf8');
    await fsp.writeFile(stderrPath, '', 'utf8');

    stdinFd = fs.openSync(stdinPath, 'r');
    stdoutFd = fs.openSync(stdoutPath, 'w+');
    stderrFd = fs.openSync(stderrPath, 'w+');

    const wasi = createWasiInstance({
      args: [`connector:${connectorId}`, `action:${action.name}`, ...(action.commandArgs || [])],
      env: {
        STATESET_CONNECTOR_ID: connectorId,
        STATESET_CONNECTOR_VERSION: connectorVersion,
        STATESET_CONNECTOR_ACTION: action.name,
      },
      preopens: {
        '/workspace': workingDirectory,
      },
      stdin: stdinFd,
      stdout: stdoutFd,
      stderr: stderrFd,
      returnOnExit: true,
    });
    const { module } = await compileWasmModule(modulePath);
    const instance = await WebAssembly.instantiate(module, getWasiImportObject(wasi));
    if (typeof instance.exports._start === 'function') {
      wasi.start(instance);
    } else if (typeof instance.exports._initialize === 'function') {
      if (typeof wasi.initialize === 'function') {
        wasi.initialize(instance);
      } else {
        instance.exports._initialize();
      }
    } else {
      throw new Error('WASI connector module must export "_start" or "_initialize".');
    }

    const elapsedMs = Date.now() - startedAt;
    if (Number.isFinite(timeoutMs) && timeoutMs > 0 && elapsedMs > timeoutMs) {
      throw new Error(`Connector action exceeded timeout (${elapsedMs}ms > ${timeoutMs}ms).`);
    }
    const stdoutText = (await fsp.readFile(stdoutPath, 'utf8')).trim();
    const stderrText = (await fsp.readFile(stderrPath, 'utf8')).trim();

    let parsedOutput = null;
    if (stdoutText.length > 0) {
      try {
        parsedOutput = JSON.parse(stdoutText);
      } catch {
        parsedOutput = { raw: stdoutText };
      }
    } else {
      parsedOutput = { success: true, output: null };
    }

    return {
      output: parsedOutput,
      execution: {
        elapsedMs,
        stderr: stderrText || null,
      },
    };
  } finally {
    if (stdinFd !== null) fs.closeSync(stdinFd);
    if (stdoutFd !== null) fs.closeSync(stdoutFd);
    if (stderrFd !== null) fs.closeSync(stderrFd);
    await fsp.rm(tempRoot, { recursive: true, force: true });
  }
}

export function getConnectorHome(options = {}) {
  const explicitHome = options.connectorHome || process.env.STATESET_CONNECTOR_HOME;
  if (explicitHome) return path.resolve(explicitHome);
  return path.resolve(process.cwd(), '.stateset', 'connectors');
}

export async function listConnectorMarketplace({
  connectorHome,
  connectorId = null,
  query = null,
  tag = null,
  limit = 100,
} = {}) {
  const resolvedHome = getConnectorHome({ connectorHome });
  const catalog = await loadCatalog(resolvedHome);
  const normalizedId = connectorId ? normalizeConnectorId(connectorId) : null;
  const normalizedQuery = query ? String(query).toLowerCase() : null;
  const normalizedTag = tag ? String(tag).toLowerCase() : null;
  const max = Math.max(1, Math.min(Number(limit) || 100, 500));

  let entries = Array.isArray(catalog.connectors) ? catalog.connectors.slice() : [];
  if (normalizedId) {
    entries = entries.filter((entry) => entry.id === normalizedId);
  }
  if (normalizedTag) {
    entries = entries.filter((entry) =>
      Array.isArray(entry.tags)
        ? entry.tags.some((entryTag) => String(entryTag).toLowerCase() === normalizedTag)
        : false,
    );
  }
  if (normalizedQuery) {
    entries = entries.filter((entry) => {
      const haystack = `${entry.id} ${entry.name || ''} ${entry.description || ''} ${(entry.tags || []).join(' ')}`;
      return haystack.toLowerCase().includes(normalizedQuery);
    });
  }
  entries.sort((left, right) => {
    const idCmp = String(left.id).localeCompare(String(right.id));
    if (idCmp !== 0) return idCmp;
    return semverLikeCompare(String(right.version), String(left.version));
  });

  return {
    success: true,
    connectorHome: resolvedHome,
    total: entries.length,
    connectors: entries.slice(0, max),
  };
}

export async function publishConnector({
  connectorHome,
  connectorId,
  version = '0.1.0',
  name = null,
  description = null,
  wasmPath,
  runtimeKind = 'native-export',
  actions = [],
  tags = [],
  publisher = null,
  force = false,
} = {}) {
  const resolvedHome = getConnectorHome({ connectorHome });
  const id = normalizeConnectorId(connectorId);
  const resolvedVersion = normalizeVersion(version);
  const resolvedRuntimeKind = normalizeRuntimeKind(runtimeKind);
  const resolvedWasmPath = path.resolve(String(wasmPath || ''));
  if (!(await pathExists(resolvedWasmPath))) {
    throw new Error(`WASM file does not exist: ${resolvedWasmPath}`);
  }

  const wasmSha256 = await sha256File(resolvedWasmPath);
  let normalizedActions = normalizeConnectorActions(actions);
  if (resolvedRuntimeKind === 'native-export') {
    normalizedActions = await resolveNativeExportActions({
      wasmPath: resolvedWasmPath,
      declaredActions: normalizedActions,
    });
  } else if (normalizedActions.length === 0) {
    normalizedActions = [
      {
        name: 'run',
        exportName: null,
        description: 'Invoke WASI command connector',
        args: [],
        commandArgs: [],
        timeoutMs: null,
        inputSchema: null,
      },
    ];
  }

  const catalog = await loadCatalog(resolvedHome);
  const existingIndex = catalog.connectors.findIndex(
    (entry) => entry.id === id && entry.version === resolvedVersion,
  );
  if (existingIndex >= 0 && !force) {
    throw new Error(
      `Connector "${id}@${resolvedVersion}" already exists in catalog. Use force=true to overwrite.`,
    );
  }

  const entry = {
    schemaVersion: CONNECTOR_SCHEMA_VERSION,
    id,
    name: name || id,
    version: resolvedVersion,
    description: description || null,
    runtime: { kind: resolvedRuntimeKind },
    actions: normalizedActions,
    tags: Array.isArray(tags) ? tags.map((value) => String(value).trim()).filter(Boolean) : [],
    publisher: publisher ? String(publisher) : null,
    wasmSha256,
    source: {
      type: 'file',
      wasmPath: resolvedWasmPath,
    },
    publishedAt: new Date().toISOString(),
  };
  entry.attestation = buildConnectorAttestation({ entry });
  entry.safetyAssessment = assessConnectorEntrySafety({
    entry,
    attestationVerification: verifyConnectorEntryAttestation({ entry }),
  });

  if (existingIndex >= 0) {
    catalog.connectors[existingIndex] = entry;
  } else {
    catalog.connectors.push(entry);
  }
  await saveCatalog(resolvedHome, catalog);

  return {
    success: true,
    connector: entry,
  };
}

export async function signConnectorAttestation({
  connectorHome,
  connectorId,
  version = null,
  keyId = null,
  signedBy = null,
  signingKey = null,
} = {}) {
  const resolvedHome = getConnectorHome({ connectorHome });
  const key = resolveConnectorSigningKey(signingKey);
  if (!key) {
    throw new Error(
      'Connector signing key is required. Set STATESET_CONNECTOR_SIGNING_KEY before signing attestations.',
    );
  }

  const catalog = await loadCatalog(resolvedHome);
  const { entry, index } = resolveCatalogConnectorEntry(catalog, connectorId, version);
  const attestation = buildConnectorAttestation({
    entry,
    signingKey: key,
    keyId,
    signedBy,
  });
  const updated = { ...entry, attestation };
  updated.safetyAssessment = assessConnectorEntrySafety({
    entry: updated,
    attestationVerification: verifyConnectorEntryAttestation({
      entry: updated,
      signingKey: key,
    }),
    signingKey: key,
  });
  catalog.connectors[index] = updated;
  await saveCatalog(resolvedHome, catalog);

  return {
    success: true,
    connectorHome: resolvedHome,
    connector: updated,
    verification: verifyConnectorEntryAttestation({ entry: updated, signingKey: key }),
  };
}

export async function verifyConnectorAttestation({
  connectorHome,
  connectorId,
  version = null,
  signingKey = null,
} = {}) {
  const resolvedHome = getConnectorHome({ connectorHome });
  const catalog = await loadCatalog(resolvedHome);
  const { entry } = resolveCatalogConnectorEntry(catalog, connectorId, version);
  const verification = verifyConnectorEntryAttestation({ entry, signingKey });

  return {
    success: true,
    connectorHome: resolvedHome,
    connector: {
      id: entry.id,
      version: entry.version,
      name: entry.name || entry.id,
      publisher: entry.publisher || null,
      attestation: entry.attestation || null,
      certification: entry.certification || null,
      safetyAssessment: entry.safetyAssessment || null,
    },
    verification,
  };
}

export async function assessConnectorSafety({
  connectorHome,
  connectorId,
  version = null,
  signingKey = null,
} = {}) {
  const resolvedHome = getConnectorHome({ connectorHome });
  const catalog = await loadCatalog(resolvedHome);
  const { entry } = resolveCatalogConnectorEntry(catalog, connectorId, version);
  const verification = verifyConnectorEntryAttestation({ entry, signingKey });
  const safetyAssessment = assessConnectorEntrySafety({
    entry,
    attestationVerification: verification,
    signingKey,
  });

  return {
    success: true,
    connectorHome: resolvedHome,
    connector: {
      id: entry.id,
      version: entry.version,
      name: entry.name || entry.id,
      publisher: entry.publisher || null,
      runtime: entry.runtime || null,
      certification: entry.certification || null,
    },
    verification,
    safetyAssessment,
  };
}

export async function certifyConnector({
  connectorHome,
  connectorId,
  version = null,
  status = 'certified',
  level = null,
  assessor = null,
  notes = null,
  minSafetyScore = 70,
  force = false,
  signingKey = null,
} = {}) {
  const resolvedHome = getConnectorHome({ connectorHome });
  const catalog = await loadCatalog(resolvedHome);
  const { entry, index } = resolveCatalogConnectorEntry(catalog, connectorId, version);
  const verification = verifyConnectorEntryAttestation({ entry, signingKey });
  const safetyAssessment = assessConnectorEntrySafety({
    entry,
    attestationVerification: verification,
    signingKey,
  });
  const resolvedStatus = normalizeCertificationStatus(status);
  const resolvedMinScore =
    minSafetyScore === null || minSafetyScore === undefined
      ? null
      : clampSafetyScore(minSafetyScore);

  if (!force && resolvedStatus === 'certified') {
    if (!verification.valid) {
      throw new Error(
        `Cannot certify connector "${entry.id}@${entry.version}": attestation verification failed (${verification.reason || 'invalid_attestation'}).`,
      );
    }
    if (resolvedMinScore !== null && safetyAssessment.score < resolvedMinScore) {
      throw new Error(
        `Cannot certify connector "${entry.id}@${entry.version}": safety score ${safetyAssessment.score} is below minimum ${resolvedMinScore}.`,
      );
    }
    if (safetyAssessment.recommendation === 'block') {
      throw new Error(
        `Cannot certify connector "${entry.id}@${entry.version}": safety recommendation is "block".`,
      );
    }
  }

  const resolvedLevel =
    level && String(level).trim()
      ? String(level).trim().toLowerCase()
      : safetyAssessment.score >= 90
        ? 'platinum'
        : safetyAssessment.score >= 80
          ? 'gold'
          : safetyAssessment.score >= 70
            ? 'silver'
            : 'bronze';

  const certification = {
    schemaVersion: CONNECTOR_CERTIFICATION_SCHEMA_VERSION,
    policyVersion: CONNECTOR_SAFETY_POLICY_VERSION,
    status: resolvedStatus,
    level: resolvedLevel,
    safetyScore: safetyAssessment.score,
    tier: safetyAssessment.tier,
    recommendation: safetyAssessment.recommendation,
    assessor: assessor ? String(assessor) : 'stateset-autonomous-certifier',
    notes: notes ? String(notes) : null,
    attestationAlgorithm: verification.algorithm || null,
    attestationVerified: verification.valid,
    issuedAt: new Date().toISOString(),
    risks: safetyAssessment.risks || [],
  };

  const updated = {
    ...entry,
    certification,
    safetyAssessment,
  };
  catalog.connectors[index] = updated;
  await saveCatalog(resolvedHome, catalog);

  return {
    success: true,
    connectorHome: resolvedHome,
    connector: updated,
    verification,
    safetyAssessment,
    certification,
  };
}

export async function installConnector({
  connectorHome,
  connectorId,
  version = null,
  force = false,
  verifyStrict = null,
  requireCertified = null,
  minSafetyScore = null,
} = {}) {
  const resolvedHome = getConnectorHome({ connectorHome });
  const catalog = await loadCatalog(resolvedHome);
  const { entry: selected } = resolveCatalogConnectorEntry(catalog, connectorId, version);
  const strict = isStrictConnectorVerificationEnabled(verifyStrict);
  const certificationRequired = isConnectorCertificationRequired(requireCertified);
  const requiredMinSafetyScore = resolveConnectorMinSafetyScore(minSafetyScore);
  const attestationVerification = verifyConnectorEntryAttestation({ entry: selected });
  const safetyAssessment = assessConnectorEntrySafety({
    entry: selected,
    attestationVerification,
  });
  assertConnectorAttestationPolicy({
    verification: attestationVerification,
    strict,
    connectorId: selected.id,
    version: selected.version,
    operation: 'install',
  });
  assertConnectorCertificationPolicy({
    entry: selected,
    safetyAssessment,
    requireCertified: certificationRequired,
    minSafetyScore: requiredMinSafetyScore,
    connectorId: selected.id,
    version: selected.version,
    operation: 'install',
  });

  const sourceWasmPath = path.resolve(selected?.source?.wasmPath || '');
  if (!(await pathExists(sourceWasmPath))) {
    throw new Error(
      `Connector source WASM file is missing for "${selected.id}@${selected.version}": ${sourceWasmPath}`,
    );
  }

  const installDir = getInstalledVersionDir(resolvedHome, selected.id, selected.version);
  const manifestPath = getManifestPath(resolvedHome, selected.id, selected.version);
  const modulePath = getModulePath(resolvedHome, selected.id, selected.version);
  if ((await pathExists(manifestPath)) && !force) {
    throw new Error(
      `Connector "${selected.id}@${selected.version}" is already installed. Use force=true to reinstall.`,
    );
  }

  await ensureDir(installDir);
  await fsp.copyFile(sourceWasmPath, modulePath);
  const installedSha = await sha256File(modulePath);
  if (selected.wasmSha256 && selected.wasmSha256 !== installedSha) {
    throw new Error(
      `Connector "${selected.id}@${selected.version}" failed integrity verification: expected ${selected.wasmSha256}, got ${installedSha}.`,
    );
  }

  const manifest = {
    schemaVersion: CONNECTOR_SCHEMA_VERSION,
    id: selected.id,
    name: selected.name || selected.id,
    version: selected.version,
    description: selected.description || null,
    runtime: selected.runtime || { kind: 'native-export' },
    actions: Array.isArray(selected.actions) ? selected.actions : [],
    tags: Array.isArray(selected.tags) ? selected.tags : [],
    publisher: selected.publisher || null,
    wasmSha256: selected.wasmSha256 || installedSha,
    installedWasmSha256: installedSha,
    source: selected.source || null,
    attestation: selected.attestation || null,
    certification: selected.certification || null,
    safetyAssessment,
    attestationVerification: {
      ...attestationVerification,
      strict,
      checkedAt: new Date().toISOString(),
    },
    certificationPolicy: {
      requireCertified: certificationRequired,
      minSafetyScore: requiredMinSafetyScore,
      checkedAt: new Date().toISOString(),
    },
    installedAt: new Date().toISOString(),
  };
  await writeJson(manifestPath, manifest);

  return {
    success: true,
    connector: {
      ...manifest,
      modulePath,
      manifestPath,
    },
  };
}

export async function uninstallConnector({ connectorHome, connectorId, version = null } = {}) {
  const resolvedHome = getConnectorHome({ connectorHome });
  const id = normalizeConnectorId(connectorId);
  const manifest = await resolveInstalledManifest(resolvedHome, id, version);
  const versionDir = path.dirname(manifest.manifestPath);
  await fsp.rm(versionDir, { recursive: true, force: true });
  return {
    success: true,
    removed: {
      connectorId: id,
      version: manifest.version,
      path: versionDir,
    },
  };
}

export async function listInstalledConnectors({ connectorHome, connectorId = null } = {}) {
  const resolvedHome = getConnectorHome({ connectorHome });
  const root = getInstalledRoot(resolvedHome);
  if (!(await pathExists(root))) {
    return { success: true, connectorHome: resolvedHome, total: 0, connectors: [] };
  }

  const normalizedId = connectorId ? normalizeConnectorId(connectorId) : null;
  const connectorDirs = await fsp.readdir(root, { withFileTypes: true });
  const manifests = [];

  for (const connectorDir of connectorDirs) {
    if (!connectorDir.isDirectory()) continue;
    if (normalizedId && connectorDir.name !== normalizedId) continue;

    const versionRoot = path.join(root, connectorDir.name);
    const versionDirs = await fsp.readdir(versionRoot, { withFileTypes: true });
    for (const versionDir of versionDirs) {
      if (!versionDir.isDirectory()) continue;
      const manifest = await readInstalledManifest(
        resolvedHome,
        connectorDir.name,
        versionDir.name,
      );
      if (!manifest) continue;
      manifests.push({
        ...manifest,
        connectorId: connectorDir.name,
      });
    }
  }

  manifests.sort((left, right) => {
    const idCmp = String(left.id).localeCompare(String(right.id));
    if (idCmp !== 0) return idCmp;
    return semverLikeCompare(String(right.version), String(left.version));
  });

  return {
    success: true,
    connectorHome: resolvedHome,
    total: manifests.length,
    connectors: manifests,
  };
}

export async function getInstalledConnector({ connectorHome, connectorId, version = null } = {}) {
  const resolvedHome = getConnectorHome({ connectorHome });
  const manifest = await resolveInstalledManifest(resolvedHome, connectorId, version);
  return {
    success: true,
    connectorHome: resolvedHome,
    connector: manifest,
  };
}

export async function executeInstalledConnectorAction({
  connectorHome,
  connectorId,
  version = null,
  action,
  params = {},
  context = {},
  timeoutMs = null,
  verifyStrict = null,
  requireCertified = null,
  minSafetyScore = null,
} = {}) {
  const resolvedHome = getConnectorHome({ connectorHome });
  const manifest = await resolveInstalledManifest(resolvedHome, connectorId, version);
  const strict = isStrictConnectorVerificationEnabled(verifyStrict);
  const certificationRequired = isConnectorCertificationRequired(requireCertified);
  const requiredMinSafetyScore = resolveConnectorMinSafetyScore(minSafetyScore);
  const attestationVerification = verifyConnectorEntryAttestation({ entry: manifest });
  const safetyAssessment = assessConnectorEntrySafety({
    entry: manifest,
    attestationVerification,
  });
  assertConnectorAttestationPolicy({
    verification: attestationVerification,
    strict,
    connectorId: manifest.id,
    version: manifest.version,
    operation: 'execute',
  });
  assertConnectorCertificationPolicy({
    entry: manifest,
    safetyAssessment,
    requireCertified: certificationRequired,
    minSafetyScore: requiredMinSafetyScore,
    connectorId: manifest.id,
    version: manifest.version,
    operation: 'execute',
  });
  const actionName = normalizeActionName(action);
  const actionDef = (manifest.actions || []).find((entry) => entry?.name === actionName);
  if (!actionDef) {
    throw new Error(
      `Connector "${manifest.id}@${manifest.version}" does not expose action "${actionName}".`,
    );
  }

  const runtimeKind = normalizeRuntimeKind(manifest?.runtime?.kind || 'native-export');
  const effectiveTimeoutMs =
    timeoutMs === null || timeoutMs === undefined ? actionDef.timeoutMs || null : Number(timeoutMs);
  const modulePath =
    manifest.modulePath || getModulePath(resolvedHome, manifest.id, manifest.version);
  if (!(await pathExists(modulePath))) {
    throw new Error(`Installed connector module is missing: ${modulePath}`);
  }

  const result =
    runtimeKind === 'wasi-command'
      ? await executeWasiCommandAction({
          modulePath,
          connectorId: manifest.id,
          connectorVersion: manifest.version,
          action: actionDef,
          params: params || {},
          context: context || {},
          timeoutMs: effectiveTimeoutMs,
          workingDirectory: path.dirname(modulePath),
        })
      : await executeNativeExportAction({
          modulePath,
          action: actionDef,
          params: params || {},
          timeoutMs: effectiveTimeoutMs,
        });

  return {
    success: true,
    connector: {
      id: manifest.id,
      name: manifest.name,
      version: manifest.version,
      runtime: runtimeKind,
      tags: manifest.tags || [],
      attestationVerified: attestationVerification.valid,
      certification: manifest.certification || null,
      safetyAssessment,
    },
    action: actionDef.name,
    output: result.output,
    execution: {
      requestId: randomUUID(),
      runtime: runtimeKind,
      timeoutMs: effectiveTimeoutMs,
      attestation: {
        ...attestationVerification,
        strict,
      },
      certificationPolicy: {
        requireCertified: certificationRequired,
        minSafetyScore: requiredMinSafetyScore,
      },
      ...result.execution,
    },
  };
}

export function __resetWasmConnectorState() {
  wasmModuleCache.clear();
}
