import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  businessProfileDoctor,
  diffBusinessProfiles,
  initBusinessProfile,
  loadBusinessProfile,
  writeBusinessProfile,
} from '../../src/business-profile.js';

function tempProject() {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-profile-'));
}

test('business profile init creates a valid portable declaration', () => {
  const root = tempProject();
  const file = initBusinessProfile(root, {
    profile: { business: { name: 'Acme', currency: 'CAD' } },
  });
  assert.equal(file, path.join(root, '.stateset', 'business.yaml'));
  const loaded = loadBusinessProfile(root);
  assert.equal(loaded.errors.length, 0);
  assert.equal(loaded.profile.business.currency, 'CAD');
  assert.equal(businessProfileDoctor(root).ready, true);
});

test('invalid declarations are rejected before they can be installed', () => {
  const root = tempProject();
  assert.throws(
    () => writeBusinessProfile({ schemaVersion: 1, business: { name: '', currency: 'dollars' } }, root),
    /business\.name.*business\.currency/s,
  );
});

test('profile diff is deterministic and reports changed leaves', () => {
  const changes = diffBusinessProfiles(
    { business: { currency: 'USD' }, modules: { inventory: true } },
    { business: { currency: 'CAD' }, modules: { inventory: false } },
  );
  assert.deepEqual(changes, [
    { path: 'business.currency', before: '"USD"', after: '"CAD"' },
    { path: 'modules.inventory', before: 'true', after: 'false' },
  ]);
});
