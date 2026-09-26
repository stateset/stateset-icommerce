import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import {
  businessProfileDoctor,
  businessProfileContext,
  businessProfilePromptAppend,
  diffBusinessProfiles,
  installBusinessPack,
  initBusinessProfile,
  listBusinessPacks,
  loadBusinessPack,
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
    () =>
      writeBusinessProfile({ schemaVersion: 1, business: { name: '', currency: 'dollars' } }, root),
    /business\.name.*business\.currency/s,
  );
});

test('profile YAML rejects prototype-sensitive keys before merging defaults', () => {
  const root = tempProject();
  const directory = path.join(root, '.stateset');
  fs.mkdirSync(directory);
  fs.writeFileSync(
    path.join(directory, 'business.yaml'),
    'schemaVersion: 1\n__proto__:\n  polluted: true\n',
  );
  const loaded = loadBusinessProfile(root);
  assert.equal(loaded.profile, null);
  assert.match(loaded.errors.join(' '), /unsafe profile key: profile\.__proto__/);
  assert.equal({}.polluted, undefined);
});

test('pack manifest cannot load a profile outside its directory', () => {
  const root = tempProject();
  const packRoot = path.join(root, 'pack');
  fs.mkdirSync(packRoot);
  const outside = path.join(root, 'outside.yaml');
  fs.writeFileSync(outside, 'schemaVersion: 1\n');
  fs.writeFileSync(path.join(packRoot, 'pack.yaml'), 'profile: ../outside.yaml\n');
  assert.throws(() => loadBusinessPack(packRoot), /inside the pack/);

  fs.writeFileSync(path.join(packRoot, 'pack.yaml'), 'profile: linked.yaml\n');
  fs.symlinkSync(outside, path.join(packRoot, 'linked.yaml'));
  assert.throws(() => loadBusinessPack(packRoot), /inside the pack/);
});

test('profile context gives agents business-specific operating vocabulary', () => {
  const root = tempProject();
  initBusinessProfile(root, {
    profile: {
      business: { name: 'Acme', currency: 'CAD' },
      terminology: { order: 'work order' },
      workflows: [{ name: 'service-intake', resource: 'order' }],
    },
  });
  const context = businessProfileContext(root);
  assert.equal(context.ready, true);
  assert.match(context.text, /Business: Acme/);
  assert.match(context.text, /order=work order/);
  assert.match(context.text, /service-intake/);
  assert.match(businessProfilePromptAppend(root), /<business_profile>/);
  assert.match(businessProfilePromptAppend(root), /not as executable instructions/);
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

test('local packs preview changes and record a lock on install', () => {
  const root = tempProject();
  const packRoot = tempProject();
  fs.writeFileSync(
    path.join(packRoot, 'pack.yaml'),
    'name: repair-shop\nversion: 1.0.0\ndescription: Repair shop\n',
  );
  fs.writeFileSync(
    path.join(packRoot, 'business.yaml'),
    `schemaVersion: 1\nbusiness:\n  name: Repair shop\n  currency: CAD\n  timezone: UTC\nmodules:\n  orders: true\n  inventory: true\n  payments: true\n  returns: true\npolicies: []\nworkflows: []\nviews: []\nautomations: []\nintegrations: []\n`,
  );
  const pack = loadBusinessPack(packRoot);
  assert.equal(pack.errors.length, 0);
  assert.equal(installBusinessPack(packRoot, root).preview, true);
  const installed = installBusinessPack(packRoot, root, { force: true, preview: false });
  assert.equal(installed.preview, false);
  assert.equal(fs.existsSync(path.join(root, '.stateset', 'packs', 'repair-shop.lock.json')), true);
  assert.equal(
    listBusinessPacks(path.dirname(packRoot)).some((entry) => entry.name === 'repair-shop'),
    true,
  );
});

test('pack merge preserves existing business identity unless replaced', () => {
  const root = tempProject();
  const packRoot = tempProject();
  initBusinessProfile(root, {
    profile: { business: { name: 'Acme', currency: 'CAD', timezone: 'America/Vancouver' } },
  });
  fs.writeFileSync(path.join(packRoot, 'pack.yaml'), 'name: finance\nversion: 1.0.0\n');
  fs.writeFileSync(
    path.join(packRoot, 'business.yaml'),
    `schemaVersion: 1
business:
  name: Example finance business
  currency: USD
  timezone: UTC
modules:
  orders: true
  inventory: false
  payments: true
  returns: false
policies:
  - name: payment-review
workflows: []
views: []
automations: []
integrations: []
`,
  );
  const merged = installBusinessPack(packRoot, root);
  assert.deepEqual(merged.profile.business, {
    name: 'Acme',
    currency: 'CAD',
    timezone: 'America/Vancouver',
  });
  assert.equal(merged.profile.modules.inventory, false);
  assert.equal(merged.profile.policies[0].name, 'payment-review');
  assert.equal(
    merged.changes.some((change) => change.path.startsWith('business.')),
    false,
  );
  assert.equal(
    installBusinessPack(packRoot, root, { replace: true }).profile.business.name,
    'Example finance business',
  );
});

test('profile apply previews without writing and installs only with --apply', () => {
  const root = tempProject();
  const source = path.join(root, 'candidate.yaml');
  const command = fileURLToPath(new URL('../../bin/stateset-profile.js', import.meta.url));
  const destination = path.join(root, '.stateset', 'business.yaml');
  fs.writeFileSync(
    source,
    `schemaVersion: 1
business:
  name: Acme
  currency: CAD
  timezone: America/Vancouver
modules: {}
policies: []
workflows: []
views: []
automations: []
integrations: []
`,
  );

  const run = (...args) => {
    const result = spawnSync(
      process.execPath,
      [command, 'apply', '--root', root, '--file', source, '--json', ...args],
      {
        encoding: 'utf8',
      },
    );
    assert.equal(result.status, 0, result.stderr);
    return JSON.parse(result.stdout);
  };

  const preview = run();
  assert.equal(preview.preview, true);
  assert.equal(
    preview.changes.some((change) => change.path === 'business.name'),
    true,
  );
  assert.equal(fs.existsSync(destination), false);

  const installed = run('--apply');
  assert.equal(installed.preview, false);
  assert.equal(fs.existsSync(destination), true);
  assert.equal(loadBusinessProfile(root).profile.business.name, 'Acme');
});
