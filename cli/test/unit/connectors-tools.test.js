import { afterEach, beforeEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { connectorTools } from '../../src/tools/connectors.js';
import { __resetWasmConnectorState } from '../../src/connectors/wasm-marketplace.js';

function findTool(name) {
  const tool = connectorTools.find((entry) => entry.name === name);
  if (!tool) {
    throw new Error(`Tool "${name}" not found in connectorTools`);
  }
  return tool;
}

const ADD_WASM_BYTES = Buffer.from([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x01,
  0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64, 0x00, 0x00, 0x0a, 0x09,
  0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b,
]);

describe('connector tools', () => {
  let connectorHome;
  let previousHome;
  let previousSigningKey;
  let previousStrictVerify;
  let previousRequireCertified;
  let previousMinSafetyScore;

  beforeEach(async () => {
    __resetWasmConnectorState();
    previousHome = process.env.STATESET_CONNECTOR_HOME;
    previousSigningKey = process.env.STATESET_CONNECTOR_SIGNING_KEY;
    previousStrictVerify = process.env.STATESET_CONNECTOR_VERIFY_STRICT;
    previousRequireCertified = process.env.STATESET_CONNECTOR_REQUIRE_CERTIFIED;
    previousMinSafetyScore = process.env.STATESET_CONNECTOR_MIN_SAFETY_SCORE;
    delete process.env.STATESET_CONNECTOR_SIGNING_KEY;
    delete process.env.STATESET_CONNECTOR_VERIFY_STRICT;
    delete process.env.STATESET_CONNECTOR_REQUIRE_CERTIFIED;
    delete process.env.STATESET_CONNECTOR_MIN_SAFETY_SCORE;
    connectorHome = await mkdtemp(path.join(tmpdir(), 'stateset-connectors-test-'));
    process.env.STATESET_CONNECTOR_HOME = connectorHome;
  });

  afterEach(async () => {
    __resetWasmConnectorState();
    if (previousHome === undefined) {
      delete process.env.STATESET_CONNECTOR_HOME;
    } else {
      process.env.STATESET_CONNECTOR_HOME = previousHome;
    }
    if (previousSigningKey === undefined) {
      delete process.env.STATESET_CONNECTOR_SIGNING_KEY;
    } else {
      process.env.STATESET_CONNECTOR_SIGNING_KEY = previousSigningKey;
    }
    if (previousStrictVerify === undefined) {
      delete process.env.STATESET_CONNECTOR_VERIFY_STRICT;
    } else {
      process.env.STATESET_CONNECTOR_VERIFY_STRICT = previousStrictVerify;
    }
    if (previousRequireCertified === undefined) {
      delete process.env.STATESET_CONNECTOR_REQUIRE_CERTIFIED;
    } else {
      process.env.STATESET_CONNECTOR_REQUIRE_CERTIFIED = previousRequireCertified;
    }
    if (previousMinSafetyScore === undefined) {
      delete process.env.STATESET_CONNECTOR_MIN_SAFETY_SCORE;
    } else {
      process.env.STATESET_CONNECTOR_MIN_SAFETY_SCORE = previousMinSafetyScore;
    }
    await rm(connectorHome, { recursive: true, force: true });
  });

  it('exposes expected connector ecosystem tools', () => {
    const names = connectorTools.map((tool) => tool.name);
    for (const expected of [
      'list_connector_marketplace',
      'publish_wasm_connector',
      'install_wasm_connector',
      'assess_wasm_connector_safety',
      'certify_wasm_connector',
      'sign_wasm_connector_attestation',
      'verify_wasm_connector_attestation',
      'uninstall_wasm_connector',
      'list_installed_connectors',
      'get_installed_connector',
      'execute_wasm_connector',
    ]) {
      assert.ok(names.includes(expected), `missing connector tool: ${expected}`);
    }
  });

  it('publish_wasm_connector requires --apply', async () => {
    const wasmPath = path.join(connectorHome, 'add.wasm');
    await writeFile(wasmPath, ADD_WASM_BYTES);
    const publishTool = findTool('publish_wasm_connector');
    const result = await publishTool.handler({
      allowApply: false,
      params: {
        connectorId: 'math.add',
        version: '1.0.0',
        wasmPath,
      },
    });
    assert.equal(result.success, false);
    assert.ok(String(result.error).includes('--apply'));
  });

  it('certify_wasm_connector requires --apply', async () => {
    const wasmPath = path.join(connectorHome, 'add.wasm');
    await writeFile(wasmPath, ADD_WASM_BYTES);
    const publishTool = findTool('publish_wasm_connector');
    const certifyTool = findTool('certify_wasm_connector');
    await publishTool.handler({
      allowApply: true,
      params: {
        connectorId: 'math.add',
        version: '0.9.0',
        wasmPath,
        runtimeKind: 'native-export',
        actions: [{ name: 'add', exportName: 'add', args: ['a', 'b'] }],
      },
    });
    const result = await certifyTool.handler({
      allowApply: false,
      params: {
        connectorId: 'math.add',
        version: '0.9.0',
      },
    });
    assert.equal(result.success, false);
    assert.ok(String(result.error).includes('--apply'));
  });

  it('assesses and certifies connector safety', async () => {
    const wasmPath = path.join(connectorHome, 'add.wasm');
    await writeFile(wasmPath, ADD_WASM_BYTES);

    const publishTool = findTool('publish_wasm_connector');
    const assessTool = findTool('assess_wasm_connector_safety');
    const certifyTool = findTool('certify_wasm_connector');

    await publishTool.handler({
      allowApply: true,
      params: {
        connectorId: 'math.add',
        version: '1.4.0',
        wasmPath,
        runtimeKind: 'native-export',
        actions: [{ name: 'add', exportName: 'add', args: ['a', 'b'] }],
      },
    });

    const assessed = await assessTool.handler({
      params: { connectorId: 'math.add', version: '1.4.0' },
    });
    assert.equal(assessed.success, true);
    assert.ok(Number.isInteger(assessed.safetyAssessment.score));
    assert.ok(assessed.safetyAssessment.score >= 0 && assessed.safetyAssessment.score <= 100);

    const certified = await certifyTool.handler({
      allowApply: true,
      params: {
        connectorId: 'math.add',
        version: '1.4.0',
        assessor: 'unit-test',
        minSafetyScore: 0,
      },
    });
    assert.equal(certified.success, true);
    assert.equal(certified.certification.status, 'certified');
    assert.equal(certified.certification.assessor, 'unit-test');
    assert.ok(Number.isInteger(certified.certification.safetyScore));
  });

  it('publishes unsigned attestations and supports signing + verification', async () => {
    const wasmPath = path.join(connectorHome, 'add.wasm');
    await writeFile(wasmPath, ADD_WASM_BYTES);

    const publishTool = findTool('publish_wasm_connector');
    const verifyTool = findTool('verify_wasm_connector_attestation');
    const signTool = findTool('sign_wasm_connector_attestation');

    await publishTool.handler({
      allowApply: true,
      params: {
        connectorId: 'math.add',
        version: '1.0.0',
        wasmPath,
        runtimeKind: 'native-export',
        actions: [{ name: 'add', exportName: 'add', args: ['a', 'b'] }],
      },
    });

    const unsignedVerification = await verifyTool.handler({
      params: {
        connectorId: 'math.add',
        version: '1.0.0',
      },
    });
    assert.equal(unsignedVerification.success, true);
    assert.equal(unsignedVerification.verification.valid, true);
    assert.equal(unsignedVerification.verification.algorithm, 'deterministic-sha256');

    process.env.STATESET_CONNECTOR_SIGNING_KEY = 'test-signing-key';
    const signed = await signTool.handler({
      allowApply: true,
      params: {
        connectorId: 'math.add',
        version: '1.0.0',
        keyId: 'k1',
        signedBy: 'unit-test',
      },
    });
    assert.equal(signed.success, true);
    assert.equal(signed.verification.valid, true);
    assert.equal(signed.connector.attestation.algorithm, 'hmac-sha256');

    const signedVerification = await verifyTool.handler({
      params: {
        connectorId: 'math.add',
        version: '1.0.0',
      },
    });
    assert.equal(signedVerification.success, true);
    assert.equal(signedVerification.verification.valid, true);
    assert.equal(signedVerification.verification.algorithm, 'hmac-sha256');
  });

  it('blocks install when strict verification is enabled and catalog attestation is invalid', async () => {
    const wasmPath = path.join(connectorHome, 'add.wasm');
    await writeFile(wasmPath, ADD_WASM_BYTES);
    process.env.STATESET_CONNECTOR_SIGNING_KEY = 'strict-install-key';

    const publishTool = findTool('publish_wasm_connector');
    const signTool = findTool('sign_wasm_connector_attestation');
    const installTool = findTool('install_wasm_connector');

    await publishTool.handler({
      allowApply: true,
      params: {
        connectorId: 'math.add',
        version: '2.0.0',
        wasmPath,
        runtimeKind: 'native-export',
        actions: [{ name: 'add', exportName: 'add', args: ['a', 'b'] }],
      },
    });
    await signTool.handler({
      allowApply: true,
      params: { connectorId: 'math.add', version: '2.0.0' },
    });

    const catalogPath = path.join(connectorHome, 'catalog.json');
    const catalog = JSON.parse(await readFile(catalogPath, 'utf8'));
    const target = catalog.connectors.find(
      (entry) => entry.id === 'math.add' && entry.version === '2.0.0',
    );
    target.description = 'tampered payload';
    await writeFile(catalogPath, `${JSON.stringify(catalog, null, 2)}\n`, 'utf8');

    process.env.STATESET_CONNECTOR_VERIFY_STRICT = '1';
    await assert.rejects(
      () =>
        installTool.handler({
          allowApply: true,
          params: { connectorId: 'math.add', version: '2.0.0' },
        }),
      /attestation verification failed/i,
    );
  });

  it('blocks execute when strict verification is enabled and installed attestation is invalid', async () => {
    const wasmPath = path.join(connectorHome, 'add.wasm');
    await writeFile(wasmPath, ADD_WASM_BYTES);

    const publishTool = findTool('publish_wasm_connector');
    const installTool = findTool('install_wasm_connector');
    const executeTool = findTool('execute_wasm_connector');

    await publishTool.handler({
      allowApply: true,
      params: {
        connectorId: 'math.add',
        version: '3.0.0',
        wasmPath,
        runtimeKind: 'native-export',
        actions: [{ name: 'add', exportName: 'add', args: ['a', 'b'] }],
      },
    });
    await installTool.handler({
      allowApply: true,
      params: {
        connectorId: 'math.add',
        version: '3.0.0',
      },
    });

    const manifestPath = path.join(
      connectorHome,
      'installed',
      'math.add',
      '3.0.0',
      'manifest.json',
    );
    const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
    manifest.attestation.signature = 'invalid-signature';
    await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');

    process.env.STATESET_CONNECTOR_VERIFY_STRICT = 'true';
    await assert.rejects(
      () =>
        executeTool.handler({
          allowApply: true,
          params: {
            connectorId: 'math.add',
            version: '3.0.0',
            action: 'add',
            params: { a: 1, b: 2 },
          },
        }),
      /attestation verification failed/i,
    );
  });

  it('blocks install when certification is required and connector is not certified', async () => {
    const wasmPath = path.join(connectorHome, 'add.wasm');
    await writeFile(wasmPath, ADD_WASM_BYTES);
    const publishTool = findTool('publish_wasm_connector');
    const installTool = findTool('install_wasm_connector');

    await publishTool.handler({
      allowApply: true,
      params: {
        connectorId: 'math.add',
        version: '4.0.0',
        wasmPath,
        runtimeKind: 'native-export',
        actions: [{ name: 'add', exportName: 'add', args: ['a', 'b'] }],
      },
    });

    await assert.rejects(
      () =>
        installTool.handler({
          allowApply: true,
          params: {
            connectorId: 'math.add',
            version: '4.0.0',
            requireCertified: true,
          },
        }),
      /certification policy failed/i,
    );
  });

  it('allows certified connector install when certification policy is enabled', async () => {
    const wasmPath = path.join(connectorHome, 'add.wasm');
    await writeFile(wasmPath, ADD_WASM_BYTES);
    const publishTool = findTool('publish_wasm_connector');
    const certifyTool = findTool('certify_wasm_connector');
    const installTool = findTool('install_wasm_connector');

    await publishTool.handler({
      allowApply: true,
      params: {
        connectorId: 'math.add',
        version: '4.1.0',
        wasmPath,
        runtimeKind: 'native-export',
        actions: [{ name: 'add', exportName: 'add', args: ['a', 'b'] }],
      },
    });
    await certifyTool.handler({
      allowApply: true,
      params: {
        connectorId: 'math.add',
        version: '4.1.0',
        minSafetyScore: 0,
      },
    });

    const installed = await installTool.handler({
      allowApply: true,
      params: {
        connectorId: 'math.add',
        version: '4.1.0',
        requireCertified: true,
      },
    });
    assert.equal(installed.success, true);
    assert.equal(installed.connector.certification.status, 'certified');
  });

  it('blocks execution when minSafetyScore policy exceeds connector score', async () => {
    const wasmPath = path.join(connectorHome, 'add.wasm');
    await writeFile(wasmPath, ADD_WASM_BYTES);
    const publishTool = findTool('publish_wasm_connector');
    const installTool = findTool('install_wasm_connector');
    const executeTool = findTool('execute_wasm_connector');

    await publishTool.handler({
      allowApply: true,
      params: {
        connectorId: 'math.add',
        version: '5.0.0',
        wasmPath,
        runtimeKind: 'native-export',
        actions: [{ name: 'add', exportName: 'add', args: ['a', 'b'] }],
      },
    });
    await installTool.handler({
      allowApply: true,
      params: {
        connectorId: 'math.add',
        version: '5.0.0',
      },
    });

    await assert.rejects(
      () =>
        executeTool.handler({
          allowApply: true,
          params: {
            connectorId: 'math.add',
            version: '5.0.0',
            action: 'add',
            params: { a: 1, b: 2 },
            minSafetyScore: 100,
          },
        }),
      /safety score .* below required minimum/i,
    );
  });

  it('publishes, installs, executes, and uninstalls a native-export connector', async () => {
    const wasmPath = path.join(connectorHome, 'add.wasm');
    await writeFile(wasmPath, ADD_WASM_BYTES);

    const publishTool = findTool('publish_wasm_connector');
    const listMarketplaceTool = findTool('list_connector_marketplace');
    const installTool = findTool('install_wasm_connector');
    const listInstalledTool = findTool('list_installed_connectors');
    const getInstalledTool = findTool('get_installed_connector');
    const executeTool = findTool('execute_wasm_connector');
    const uninstallTool = findTool('uninstall_wasm_connector');

    const published = await publishTool.handler({
      allowApply: true,
      params: {
        connectorId: 'math.add',
        version: '1.2.3',
        name: 'Math Add',
        description: 'Simple addition helper',
        wasmPath,
        runtimeKind: 'native-export',
        actions: [{ name: 'add', exportName: 'add', args: ['a', 'b'] }],
        tags: ['math', 'example'],
      },
    });
    assert.equal(published.success, true);
    assert.equal(published.connector.id, 'math.add');
    assert.equal(published.connector.version, '1.2.3');

    const marketplace = await listMarketplaceTool.handler({
      params: {
        query: 'math',
      },
    });
    assert.equal(marketplace.success, true);
    assert.equal(marketplace.total, 1);
    assert.equal(marketplace.connectors[0].id, 'math.add');

    const installed = await installTool.handler({
      allowApply: true,
      params: {
        connectorId: 'math.add',
        version: '1.2.3',
      },
    });
    assert.equal(installed.success, true);
    assert.equal(installed.connector.id, 'math.add');

    const installedList = await listInstalledTool.handler({ params: {} });
    assert.equal(installedList.success, true);
    assert.equal(installedList.total, 1);
    assert.equal(installedList.connectors[0].id, 'math.add');

    const installedDetails = await getInstalledTool.handler({
      params: { connectorId: 'math.add' },
    });
    assert.equal(installedDetails.success, true);
    assert.equal(installedDetails.connector.id, 'math.add');
    assert.equal(installedDetails.connector.runtime.kind, 'native-export');

    const execution = await executeTool.handler({
      allowApply: true,
      params: {
        connectorId: 'math.add',
        action: 'add',
        params: {
          a: 7,
          b: 5,
        },
      },
    });
    assert.equal(execution.success, true);
    assert.equal(execution.action, 'add');
    assert.equal(execution.output.value, 12);
    assert.equal(execution.connector.runtime, 'native-export');

    const removed = await uninstallTool.handler({
      allowApply: true,
      params: {
        connectorId: 'math.add',
        version: '1.2.3',
      },
    });
    assert.equal(removed.success, true);
    assert.equal(removed.removed.connectorId, 'math.add');
  });
});
