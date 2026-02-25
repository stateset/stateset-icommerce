/**
 * Security-focused unit tests for marketplace remote installs.
 */

import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import path from 'path';
import os from 'os';
import fs from 'fs';
import crypto from 'crypto';
import { MarketplaceClient } from '../../src/skills/marketplace.js';

const CREATED_DIRS = [];
const ORIGINAL_FETCH = global.fetch;
const ORIGINAL_ALLOW_INSECURE_ENV = process.env.STATESET_ALLOW_INSECURE_SKILL_DOWNLOADS;

function mkTmpDir() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-marketplace-test-'));
  CREATED_DIRS.push(dir);
  return dir;
}

function buildCatalogFile(tmpDir, skillOverrides = {}) {
  const catalogPath = path.join(tmpDir, 'marketplace.json');
  const catalog = {
    version: '1.0.0',
    generatedAt: new Date().toISOString(),
    baseUrl: 'https://skills.example.com/packages/',
    skills: [
      {
        name: 'remote-skill',
        description: 'Remote skill test fixture',
        category: 'testing',
        tags: ['test'],
        version: '1.0.0',
        downloadUrl: 'https://skills.example.com/packages/remote-skill.md',
        isPublic: true,
        hasReferences: false,
        hasScripts: false,
        updatedAt: '2026-01-01',
        ...skillOverrides,
      },
    ],
  };
  fs.writeFileSync(catalogPath, JSON.stringify(catalog, null, 2));
  return catalogPath;
}

afterEach(() => {
  global.fetch = ORIGINAL_FETCH;
  if (ORIGINAL_ALLOW_INSECURE_ENV === undefined) {
    delete process.env.STATESET_ALLOW_INSECURE_SKILL_DOWNLOADS;
  } else {
    process.env.STATESET_ALLOW_INSECURE_SKILL_DOWNLOADS = ORIGINAL_ALLOW_INSECURE_ENV;
  }
  for (const dir of CREATED_DIRS.splice(0)) {
    if (fs.existsSync(dir)) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  }
});

describe('MarketplaceClient download hardening', () => {
  it('rejects non-HTTPS remote URLs before fetch', async () => {
    const tmpDir = mkTmpDir();
    const installDir = path.join(tmpDir, 'installed');
    const bundledDir = path.join(tmpDir, 'bundled');
    const catalogPath = buildCatalogFile(tmpDir, {
      downloadUrl: 'http://skills.example.com/packages/remote-skill.md',
      downloadChecksum: 'sha256:deadbeef',
    });

    let fetchCalled = false;
    global.fetch = async () => {
      fetchCalled = true;
      throw new Error('fetch should not be called');
    };

    const client = new MarketplaceClient({ catalogPath, installDir, bundledDir });
    const result = await client.install('remote-skill');

    assert.equal(result.installed, false);
    assert.match(result.error, /HTTPS/i);
    assert.equal(fetchCalled, false);
  });

  it('rejects download URLs outside marketplace baseUrl', async () => {
    const tmpDir = mkTmpDir();
    const installDir = path.join(tmpDir, 'installed');
    const bundledDir = path.join(tmpDir, 'bundled');
    const payload = '# Remote Skill\n\nSafe content';
    const digest = crypto.createHash('sha256').update(Buffer.from(payload, 'utf8')).digest('hex');
    const catalogPath = buildCatalogFile(tmpDir, {
      downloadUrl: 'https://evil.example.net/packages/remote-skill.md',
      downloadChecksum: `sha256:${digest}`,
    });

    let fetchCalled = false;
    global.fetch = async () => {
      fetchCalled = true;
      throw new Error('fetch should not be called');
    };

    const client = new MarketplaceClient({ catalogPath, installDir, bundledDir });
    const result = await client.install('remote-skill');

    assert.equal(result.installed, false);
    assert.match(result.error, /within marketplace baseUrl/i);
    assert.equal(fetchCalled, false);
  });

  it('rejects remote installs without checksum metadata', async () => {
    const tmpDir = mkTmpDir();
    const installDir = path.join(tmpDir, 'installed');
    const bundledDir = path.join(tmpDir, 'bundled');
    const catalogPath = buildCatalogFile(tmpDir, {
      downloadChecksum: undefined,
    });

    let fetchCalled = false;
    global.fetch = async () => {
      fetchCalled = true;
      throw new Error('fetch should not be called');
    };

    const client = new MarketplaceClient({ catalogPath, installDir, bundledDir });
    const result = await client.install('remote-skill');

    assert.equal(result.installed, false);
    assert.match(result.error, /downloadChecksum/i);
    assert.equal(fetchCalled, false);
  });

  it('rejects checksum algorithm metadata mismatch before fetch', async () => {
    const tmpDir = mkTmpDir();
    const installDir = path.join(tmpDir, 'installed');
    const bundledDir = path.join(tmpDir, 'bundled');
    const catalogPath = buildCatalogFile(tmpDir, {
      downloadChecksum: 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
      checksumAlgorithm: 'sha512',
    });

    let fetchCalled = false;
    global.fetch = async () => {
      fetchCalled = true;
      throw new Error('fetch should not be called');
    };

    const client = new MarketplaceClient({ catalogPath, installDir, bundledDir });
    const result = await client.install('remote-skill');

    assert.equal(result.installed, false);
    assert.match(result.error, /does not match checksumAlgorithm/i);
    assert.equal(fetchCalled, false);
  });

  it('rejects payloads with checksum mismatch and cleans partial installs', async () => {
    const tmpDir = mkTmpDir();
    const installDir = path.join(tmpDir, 'installed');
    const bundledDir = path.join(tmpDir, 'bundled');
    const payload = '# remote skill\n\nnot trusted';
    const catalogPath = buildCatalogFile(tmpDir, {
      downloadChecksum: 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
    });

    global.fetch = async () =>
      new Response(payload, {
        status: 200,
        headers: { 'content-type': 'text/markdown' },
      });

    const client = new MarketplaceClient({ catalogPath, installDir, bundledDir });
    const result = await client.install('remote-skill');
    const targetDir = path.join(installDir, 'remote-skill');

    assert.equal(result.installed, false);
    assert.match(result.error, /Checksum mismatch/i);
    assert.equal(fs.existsSync(targetDir), false);
  });

  it('installs remote markdown skill when checksum matches', async () => {
    const tmpDir = mkTmpDir();
    const installDir = path.join(tmpDir, 'installed');
    const bundledDir = path.join(tmpDir, 'bundled');
    const payload = '# Remote Skill\n\nSafe content';
    const digest = crypto.createHash('sha256').update(Buffer.from(payload, 'utf8')).digest('hex');
    const catalogPath = buildCatalogFile(tmpDir, {
      downloadChecksum: `sha256:${digest}`,
    });

    global.fetch = async () =>
      new Response(payload, {
        status: 200,
        headers: { 'content-type': 'text/markdown' },
      });

    const client = new MarketplaceClient({ catalogPath, installDir, bundledDir });
    const result = await client.install('remote-skill');

    assert.equal(result.installed, true);
    assert.equal(
      fs.existsSync(path.join(installDir, 'remote-skill', 'SKILL.md')),
      true,
      'installed skill should include SKILL.md',
    );
  });

  it('uses fetch redirect=error for remote downloads', async () => {
    const tmpDir = mkTmpDir();
    const installDir = path.join(tmpDir, 'installed');
    const bundledDir = path.join(tmpDir, 'bundled');
    const payload = '# Remote Skill\n\nSafe content';
    const digest = crypto.createHash('sha256').update(Buffer.from(payload, 'utf8')).digest('hex');
    const catalogPath = buildCatalogFile(tmpDir, {
      downloadChecksum: `sha256:${digest}`,
    });

    /** @type {Array<{url: string, options: any}>} */
    const calls = [];
    global.fetch = async (url, options) => {
      calls.push({ url: String(url), options });
      return new Response(payload, {
        status: 200,
        headers: { 'content-type': 'text/markdown' },
      });
    };

    const client = new MarketplaceClient({ catalogPath, installDir, bundledDir });
    const result = await client.install('remote-skill');

    assert.equal(result.installed, true);
    assert.equal(calls.length, 1);
    assert.equal(calls[0].options?.redirect, 'error');
  });

  it('allows insecure download mode when env flag is enabled', async () => {
    const tmpDir = mkTmpDir();
    const installDir = path.join(tmpDir, 'installed');
    const bundledDir = path.join(tmpDir, 'bundled');
    const payload = '# Remote Skill\n\nLegacy catalog without checksum';
    const catalogPath = buildCatalogFile(tmpDir, {
      downloadChecksum: undefined,
    });
    process.env.STATESET_ALLOW_INSECURE_SKILL_DOWNLOADS = '1';

    global.fetch = async () =>
      new Response(payload, {
        status: 200,
        headers: { 'content-type': 'text/markdown' },
      });

    const client = new MarketplaceClient({ catalogPath, installDir, bundledDir });
    const result = await client.install('remote-skill');

    assert.equal(result.installed, true);
    assert.equal(
      fs.existsSync(path.join(installDir, 'remote-skill', 'SKILL.md')),
      true,
      'installed skill should include SKILL.md',
    );
  });
});
