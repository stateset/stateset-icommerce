const assert = require('assert');
const { createHash } = require('crypto');
const { test } = require('node:test');

const {
  vesTestVectorMlDsaPublicKey,
  vesTestVectorMlKemPublicKey,
} = require('../index.js');

function sha256Hex(buffer) {
  return `0x${createHash('sha256').update(buffer).digest('hex')}`;
}

test('PQC vector helpers expose the fixed-seed ML-DSA-65 public key', () => {
  const publicKey = vesTestVectorMlDsaPublicKey();
  assert.strictEqual(publicKey.length, 1952);
  assert.strictEqual(
    sha256Hex(publicKey),
    '0xe933697f7a3d671b8c294452465230d4d433d337afd25b99dba884175541a855',
  );
});

test('PQC vector helpers expose the fixed-seed ML-KEM-768 public key', () => {
  const publicKey = vesTestVectorMlKemPublicKey();
  assert.strictEqual(publicKey.length, 1184);
  assert.strictEqual(
    sha256Hex(publicKey),
    '0x63ae13ddb2f35156d69304a1783fa465e87414e38bf6aac6931797368235d296',
  );
});
