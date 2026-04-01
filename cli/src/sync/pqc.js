/**
 * PQ profile helpers shared by sync clients and config validation.
 */

export const SECURITY_PROFILE_LEGACY = 'legacy';
export const SECURITY_PROFILE_HYBRID = 'hybrid';
export const SECURITY_PROFILE_PQC_STRICT = 'pqc-strict';

const SECURITY_PROFILES = new Set([
  SECURITY_PROFILE_LEGACY,
  SECURITY_PROFILE_HYBRID,
  SECURITY_PROFILE_PQC_STRICT,
]);

export const SIGNATURE_SCHEME_UNSPECIFIED = 0;
export const SIGNATURE_SCHEME_ED25519 = 1;
export const SIGNATURE_SCHEME_ML_DSA_65 = 2;
export const SIGNATURE_SCHEME_ED25519_ML_DSA_65 = 3;

export const KEY_TYPE_SIGNING = 1;
export const KEY_TYPE_ENCRYPTION = 2;

export const KEY_ALGORITHM_UNSPECIFIED = 0;
export const KEY_ALGORITHM_ED25519 = 1;
export const KEY_ALGORITHM_X25519 = 2;
export const KEY_ALGORITHM_ML_DSA_65 = 3;
export const KEY_ALGORITHM_ML_KEM_768 = 4;
export const KEY_ALGORITHM_ED25519_ML_DSA_65 = 5;
export const KEY_ALGORITHM_X25519_ML_KEM_768 = 6;

export const KEY_WRAP_SCHEME_UNSPECIFIED = 0;
export const KEY_WRAP_SCHEME_X25519_HKDF_SHA256 = 1;
export const KEY_WRAP_SCHEME_ML_KEM_768 = 2;
export const KEY_WRAP_SCHEME_X25519_ML_KEM_768 = 3;

function hasMaterial(value) {
  if (value === null || value === undefined) {
    return false;
  }
  if (typeof value === 'string') {
    return value.length > 0;
  }
  if (value instanceof Uint8Array) {
    return value.length > 0;
  }
  return true;
}

function normalizeSignatureBundle(bundle) {
  if (!bundle) {
    return null;
  }

  return {
    ed25519Signature: bundle.ed25519Signature ?? bundle.ed25519_signature ?? null,
    mlDsa65Signature: bundle.mlDsa65Signature ?? bundle.ml_dsa_65_signature ?? null,
  };
}

function normalizePublicKeyBundle(bundle) {
  if (!bundle) {
    return null;
  }

  return {
    ed25519PublicKey: bundle.ed25519PublicKey ?? bundle.ed25519_public_key ?? null,
    mlDsa65PublicKey: bundle.mlDsa65PublicKey ?? bundle.ml_dsa_65_public_key ?? null,
    x25519PublicKey: bundle.x25519PublicKey ?? bundle.x25519_public_key ?? null,
    mlKem768PublicKey: bundle.mlKem768PublicKey ?? bundle.ml_kem_768_public_key ?? null,
  };
}

function normalizeKeyWrapParams(payloadEncrypted) {
  const params = payloadEncrypted?.keyWrapParams ?? payloadEncrypted?.key_wrap_params;
  if (!params) {
    return null;
  }

  return {
    scheme: Number(params.scheme ?? params.wrapScheme ?? params.wrap_scheme ?? 0),
    kdf: params.kdf ?? null,
    aead: params.aead ?? null,
  };
}

function normalizeRecipientWraps(payloadEncrypted) {
  const wraps = payloadEncrypted?.recipientWraps ?? payloadEncrypted?.recipient_wraps;
  if (!Array.isArray(wraps)) {
    return [];
  }

  return wraps.map((wrap) => ({
    recipientKid: Number(wrap.recipientKid ?? wrap.recipient_kid ?? 0),
    wrapScheme: Number(wrap.wrapScheme ?? wrap.wrap_scheme ?? 0),
    x25519Enc:
      wrap.x25519Enc ??
      wrap.x25519_enc ??
      wrap.x25519_enc_b64u ??
      null,
    mlKemCiphertext:
      wrap.mlKemCiphertext ??
      wrap.ml_kem_ciphertext ??
      wrap.ml_kem_ciphertext_b64u ??
      wrap.mlkem_ct_b64u ??
      null,
    wrapNonce:
      wrap.wrapNonce ??
      wrap.wrap_nonce ??
      wrap.wrap_nonce_b64u ??
      null,
    wrappedKey:
      wrap.wrappedKey ??
      wrap.wrapped_key ??
      wrap.wrapped_key_b64u ??
      wrap.ct_b64u ??
      null,
  }));
}

function inferLegacyWrapScheme(payloadEncrypted) {
  if (Array.isArray(payloadEncrypted?.recipients) && payloadEncrypted.recipients.length > 0) {
    return KEY_WRAP_SCHEME_X25519_HKDF_SHA256;
  }
  return KEY_WRAP_SCHEME_UNSPECIFIED;
}

export function getPayloadWrapScheme(payloadEncrypted) {
  const params = normalizeKeyWrapParams(payloadEncrypted);
  if (params?.scheme) {
    return params.scheme;
  }

  const wraps = normalizeRecipientWraps(payloadEncrypted);
  if (wraps.length > 0) {
    return wraps[0].wrapScheme;
  }

  return inferLegacyWrapScheme(payloadEncrypted);
}

function assertHybridSignatureBundle(event) {
  const bundle = normalizeSignatureBundle(event.agentSignatureBundle);
  if (Number(event.agentSignatureScheme ?? SIGNATURE_SCHEME_UNSPECIFIED) !== SIGNATURE_SCHEME_ED25519_ML_DSA_65) {
    throw new Error('Hybrid profile requires SIGNATURE_SCHEME_ED25519_ML_DSA_65');
  }
  if (!hasMaterial(bundle?.ed25519Signature) || !hasMaterial(bundle?.mlDsa65Signature)) {
    throw new Error('Hybrid profile requires both Ed25519 and ML-DSA-65 signature components');
  }
}

function assertStrictSignatureBundle(event) {
  const bundle = normalizeSignatureBundle(event.agentSignatureBundle);
  if (Number(event.agentSignatureScheme ?? SIGNATURE_SCHEME_UNSPECIFIED) !== SIGNATURE_SCHEME_ML_DSA_65) {
    throw new Error('pqc-strict profile requires SIGNATURE_SCHEME_ML_DSA_65');
  }
  if (!hasMaterial(bundle?.mlDsa65Signature)) {
    throw new Error('pqc-strict profile requires an ML-DSA-65 signature component');
  }
  if (hasMaterial(bundle?.ed25519Signature) || hasMaterial(event.agentSignature)) {
    throw new Error('pqc-strict profile rejects Ed25519 signature material');
  }
}

function assertHybridEncryptedPayload(payloadEncrypted) {
  const scheme = getPayloadWrapScheme(payloadEncrypted);
  if (scheme !== KEY_WRAP_SCHEME_X25519_ML_KEM_768) {
    throw new Error('Hybrid profile requires X25519+ML-KEM-768 recipient wraps');
  }

  for (const wrap of normalizeRecipientWraps(payloadEncrypted)) {
    if (wrap.wrapScheme !== KEY_WRAP_SCHEME_X25519_ML_KEM_768) {
      throw new Error('Hybrid profile requires every recipient wrap to use X25519+ML-KEM-768');
    }
    if (!hasMaterial(wrap.x25519Enc) || !hasMaterial(wrap.mlKemCiphertext) || !hasMaterial(wrap.wrappedKey)) {
      throw new Error('Hybrid profile requires x25519, ml-kem, and wrapped-key material');
    }
  }
}

function assertStrictEncryptedPayload(payloadEncrypted) {
  const scheme = getPayloadWrapScheme(payloadEncrypted);
  if (scheme !== KEY_WRAP_SCHEME_ML_KEM_768) {
    throw new Error('pqc-strict profile requires ML-KEM-768 recipient wraps');
  }

  for (const wrap of normalizeRecipientWraps(payloadEncrypted)) {
    if (wrap.wrapScheme !== KEY_WRAP_SCHEME_ML_KEM_768) {
      throw new Error('pqc-strict profile requires every recipient wrap to use ML-KEM-768');
    }
    if (!hasMaterial(wrap.mlKemCiphertext) || !hasMaterial(wrap.wrappedKey)) {
      throw new Error('pqc-strict profile requires ML-KEM ciphertext and wrapped-key material');
    }
    if (hasMaterial(wrap.x25519Enc)) {
      throw new Error('pqc-strict profile rejects X25519 recipient-wrap material');
    }
  }
}

function assertHybridKeyRegistration(keyRegistration) {
  const keyType = Number(keyRegistration.keyType ?? 0);
  const keyAlgorithm = Number(keyRegistration.keyAlgorithm ?? KEY_ALGORITHM_UNSPECIFIED);
  const bundle = normalizePublicKeyBundle(keyRegistration.publicKeyBundle);

  if (keyType === KEY_TYPE_SIGNING) {
    if (keyAlgorithm !== KEY_ALGORITHM_ED25519_ML_DSA_65) {
      throw new Error('Hybrid profile requires KEY_ALGORITHM_ED25519_ML_DSA_65 for signing keys');
    }
    if (!hasMaterial(bundle?.ed25519PublicKey) || !hasMaterial(bundle?.mlDsa65PublicKey)) {
      throw new Error('Hybrid profile requires Ed25519 and ML-DSA-65 signing public keys');
    }
    return;
  }

  if (keyType === KEY_TYPE_ENCRYPTION) {
    if (keyAlgorithm !== KEY_ALGORITHM_X25519_ML_KEM_768) {
      throw new Error('Hybrid profile requires KEY_ALGORITHM_X25519_ML_KEM_768 for encryption keys');
    }
    if (!hasMaterial(bundle?.x25519PublicKey) || !hasMaterial(bundle?.mlKem768PublicKey)) {
      throw new Error('Hybrid profile requires X25519 and ML-KEM-768 encryption public keys');
    }
    return;
  }

  throw new Error('Hybrid profile requires explicit signing or encryption key registration');
}

function assertStrictKeyRegistration(keyRegistration) {
  const keyType = Number(keyRegistration.keyType ?? 0);
  const keyAlgorithm = Number(keyRegistration.keyAlgorithm ?? KEY_ALGORITHM_UNSPECIFIED);
  const bundle = normalizePublicKeyBundle(keyRegistration.publicKeyBundle);

  if (keyType === KEY_TYPE_SIGNING) {
    if (keyAlgorithm !== KEY_ALGORITHM_ML_DSA_65) {
      throw new Error('pqc-strict profile requires KEY_ALGORITHM_ML_DSA_65 for signing keys');
    }
    if (!hasMaterial(bundle?.mlDsa65PublicKey)) {
      throw new Error('pqc-strict profile requires an ML-DSA-65 signing public key');
    }
    if (hasMaterial(bundle?.ed25519PublicKey)) {
      throw new Error('pqc-strict profile rejects Ed25519 signing public keys');
    }
    return;
  }

  if (keyType === KEY_TYPE_ENCRYPTION) {
    if (keyAlgorithm !== KEY_ALGORITHM_ML_KEM_768) {
      throw new Error('pqc-strict profile requires KEY_ALGORITHM_ML_KEM_768 for encryption keys');
    }
    if (!hasMaterial(bundle?.mlKem768PublicKey)) {
      throw new Error('pqc-strict profile requires an ML-KEM-768 encryption public key');
    }
    if (hasMaterial(bundle?.x25519PublicKey)) {
      throw new Error('pqc-strict profile rejects X25519 encryption public keys');
    }
    return;
  }

  throw new Error('pqc-strict profile requires explicit signing or encryption key registration');
}

export function resolveSecurityProfile(profile = SECURITY_PROFILE_LEGACY) {
  const normalized = String(profile ?? SECURITY_PROFILE_LEGACY).trim().toLowerCase();
  if (!SECURITY_PROFILES.has(normalized)) {
    throw new Error(
      `Unsupported sync security profile: ${profile}. Expected legacy, hybrid, or pqc-strict`,
    );
  }
  return normalized;
}

export function isSecureSequencerProtocol(protocol) {
  return protocol === 'https:' || protocol === 'grpcs:';
}

export function assertSecureTransportForProfile(profile, isSecure, transportLabel = 'sequencer transport') {
  if (resolveSecurityProfile(profile) !== SECURITY_PROFILE_LEGACY && !isSecure) {
    throw new Error(`${transportLabel} must use TLS for ${profile} sync profile`);
  }
}

export function assertEventMatchesSecurityProfile(event, profile) {
  const resolvedProfile = resolveSecurityProfile(profile);
  if (resolvedProfile === SECURITY_PROFILE_LEGACY) {
    return;
  }

  if (resolvedProfile === SECURITY_PROFILE_HYBRID) {
    assertHybridSignatureBundle(event);
  } else {
    assertStrictSignatureBundle(event);
  }

  if (Number(event.payloadKind ?? 0) !== 1) {
    return;
  }

  if (!event.payloadEncrypted) {
    throw new Error(`${resolvedProfile} profile requires payloadEncrypted for encrypted events`);
  }

  if (resolvedProfile === SECURITY_PROFILE_HYBRID) {
    assertHybridEncryptedPayload(event.payloadEncrypted);
  } else {
    assertStrictEncryptedPayload(event.payloadEncrypted);
  }
}

export function assertKeyRegistrationMatchesSecurityProfile(keyRegistration, profile) {
  const resolvedProfile = resolveSecurityProfile(profile);
  if (resolvedProfile === SECURITY_PROFILE_LEGACY) {
    return;
  }

  if (resolvedProfile === SECURITY_PROFILE_HYBRID) {
    assertHybridKeyRegistration(keyRegistration);
  } else {
    assertStrictKeyRegistration(keyRegistration);
  }
}

/**
 * Assert that a receipt signature bundle matches the active security profile.
 * @param {Object} receipt - The receipt object with signature fields.
 * @param {string} profile - The security profile ('legacy', 'hybrid', 'pqc-strict').
 * @throws {Error} If the receipt signature does not match the profile.
 */
export function assertReceiptMatchesSecurityProfile(receipt, profile) {
  const resolvedProfile = resolveSecurityProfile(profile);
  if (resolvedProfile === SECURITY_PROFILE_LEGACY) {
    return;
  }

  const bundle = normalizeSignatureBundle(receipt.signatureBundle);
  const scheme = Number(receipt.signatureScheme ?? SIGNATURE_SCHEME_UNSPECIFIED);

  if (resolvedProfile === SECURITY_PROFILE_HYBRID) {
    if (scheme !== SIGNATURE_SCHEME_ED25519_ML_DSA_65) {
      throw new Error('Hybrid profile requires SIGNATURE_SCHEME_ED25519_ML_DSA_65 for receipts');
    }
    if (!hasMaterial(bundle?.ed25519Signature) || !hasMaterial(bundle?.mlDsa65Signature)) {
      throw new Error('Hybrid profile requires both Ed25519 and ML-DSA-65 receipt signature components');
    }
  } else {
    if (scheme !== SIGNATURE_SCHEME_ML_DSA_65) {
      throw new Error('pqc-strict profile requires SIGNATURE_SCHEME_ML_DSA_65 for receipts');
    }
    if (!hasMaterial(bundle?.mlDsa65Signature)) {
      throw new Error('pqc-strict profile requires an ML-DSA-65 receipt signature component');
    }
    if (hasMaterial(bundle?.ed25519Signature)) {
      throw new Error('pqc-strict profile rejects Ed25519 receipt signature material');
    }
  }
}

/**
 * Return the profile label string for metrics recording.
 * @param {string} [profile='legacy'] - The security profile.
 * @returns {'legacy' | 'hybrid' | 'pqc-strict'}
 */
export function profileMetricLabel(profile) {
  return resolveSecurityProfile(profile);
}
