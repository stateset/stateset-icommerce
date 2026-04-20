#!/usr/bin/env node

import assert from 'node:assert/strict';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '../..');
const bindingDir = path.join(rootDir, 'bindings/node');

const REQUIRED_PACKED_FILES = [
  'README.md',
  'package.json',
  'index.js',
  'index.d.ts',
  'agent-toolkit.mjs',
  'agent-toolkit.d.ts',
  'openai.mjs',
  'openai.d.ts',
  'generic.mjs',
  'generic.d.ts',
  'langchain.mjs',
  'langchain.d.ts',
  'vercel-ai.mjs',
  'vercel-ai.d.ts',
  'toolkit-helpers.mjs',
];

function runPackDryRun() {
  const packed = spawnSync('npm', ['pack', '--dry-run', '--json'], {
    cwd: bindingDir,
    encoding: 'utf8',
  });

  assert.equal(
    packed.status,
    0,
    `npm pack --dry-run --json should succeed for bindings/node.\n${packed.stderr || packed.stdout}`,
  );

  try {
    return JSON.parse(packed.stdout);
  } catch (error) {
    throw new Error(`Unable to parse npm pack output.\n${packed.stdout}\n${error}`);
  }
}

function runPack(packDestination) {
  const packed = spawnSync('npm', ['pack', '--json', '--pack-destination', packDestination], {
    cwd: bindingDir,
    encoding: 'utf8',
  });

  assert.equal(
    packed.status,
    0,
    `npm pack --json should succeed for bindings/node.\n${packed.stderr || packed.stdout}`,
  );

  try {
    return JSON.parse(packed.stdout);
  } catch (error) {
    throw new Error(`Unable to parse npm pack output.\n${packed.stdout}\n${error}`);
  }
}

function unpackTarball(tarballPath, targetDir) {
  const extracted = spawnSync('tar', ['-xzf', tarballPath, '-C', targetDir], {
    cwd: bindingDir,
    encoding: 'utf8',
  });

  assert.equal(
    extracted.status,
    0,
    `tar should extract ${tarballPath}.\n${extracted.stderr || extracted.stdout}`,
  );
}

async function verifyPackedImports(packageDir) {
  const rootModule = await import(pathToFileURL(path.join(packageDir, 'index.js')).href);
  const Commerce = rootModule.Commerce || rootModule.default?.Commerce;
  assert.equal(typeof Commerce, 'function', 'Packed root module should expose Commerce.');
  const commerce = new Commerce(':memory:');
  assert.ok(commerce, 'Packed root module should create a Commerce instance.');

  const [openai, generic, langchain, vercelAi] = await Promise.all([
    import(pathToFileURL(path.join(packageDir, 'openai.mjs')).href),
    import(pathToFileURL(path.join(packageDir, 'generic.mjs')).href),
    import(pathToFileURL(path.join(packageDir, 'langchain.mjs')).href),
    import(pathToFileURL(path.join(packageDir, 'vercel-ai.mjs')).href),
  ]);

  const fakeToolkit = {
    getTools({ format }) {
      if (format === 'openai') {
        return [{ type: 'function', function: { name: 'list_customers' } }];
      }
      return [{ name: 'list_customers' }];
    },
    executeTool(toolName, params) {
      return { status: 'success', tool: toolName, params };
    },
    executeToolCalls(toolCalls) {
      return toolCalls.map((toolCall) => ({
        name: toolCall.function?.name || toolCall.name,
        result: { status: 'success' },
      }));
    },
    executeOpenAIToolCall(toolCall) {
      return {
        name: toolCall.function.name,
        result: { status: 'success', tool: toolCall.function.name },
      };
    },
    createToolDescriptors() {
      return [
        {
          name: 'list_customers',
          execute(params = {}) {
            return { status: 'success', params };
          },
        },
      ];
    },
    createLangChainTools() {
      return [{ name: 'list_customers' }];
    },
    createVercelAITools() {
      return { list_customers: { description: 'fake' } };
    },
  };

  const openaiTools = openai.createOpenAITools(fakeToolkit);
  assert.deepEqual(
    openaiTools.map((tool) => tool.function.name),
    ['list_customers'],
    'Packed OpenAI helper should work with a toolkit-like object without requiring the CLI peer at import time.',
  );
  const openaiExecution = await openai.executeOpenAIToolCall(fakeToolkit, {
    function: { name: 'list_customers', arguments: '{}' },
  });
  assert.equal(openaiExecution.result.status, 'success');
  const genericDescriptors = generic.createToolDescriptors(fakeToolkit);
  assert.equal(genericDescriptors[0].name, 'list_customers');
  const genericRegistry = generic.createCallableRegistry(fakeToolkit);
  assert.equal(genericRegistry.list_customers({}).status, 'success');
  const langchainTools = langchain.createLangChainTools(fakeToolkit);
  assert.equal(langchainTools[0].name, 'list_customers');
  const vercelTools = vercelAi.createVercelAITools(fakeToolkit);
  assert.ok(vercelTools.list_customers);

  try {
    await import(pathToFileURL(path.join(packageDir, 'agent-toolkit.mjs')).href);
    assert.fail('Packed agent-toolkit entrypoint should require @stateset/cli when imported in isolation.');
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    assert.match(
      message,
      /requires @stateset\/cli/,
      'Packed agent-toolkit entrypoint should fail with install guidance when the CLI peer is absent.',
    );
  }
}

async function main() {
  const packageJson = JSON.parse(readFileSync(path.join(bindingDir, 'package.json'), 'utf8'));
  const readme = readFileSync(path.join(bindingDir, 'README.md'), 'utf8');
  const packOutput = runPackDryRun();
  assert.ok(Array.isArray(packOutput) && packOutput.length > 0, 'npm pack should return at least one package descriptor.');

  assert.match(
    readme,
    /npm install @stateset\/embedded @stateset\/cli/,
    'Node binding README should explain the install pattern for the advanced agent toolkit runtime.',
  );
  assert.match(
    readme,
    /@stateset\/embedded\/agent-toolkit/,
    'Node binding README should mention the exported agent-toolkit entrypoint.',
  );

  const [pkg] = packOutput;
  const packedFiles = new Set((pkg.files || []).map((file) => file.path));

  for (const requiredFile of REQUIRED_PACKED_FILES) {
    assert.ok(packedFiles.has(requiredFile), `Packed Node binding is missing ${requiredFile}.`);
  }

  assert.ok(
    Array.from(packedFiles).some((file) => file.endsWith('.node')),
    'Packed Node binding should include at least one native .node binary.',
  );

  for (const [subpath, target] of Object.entries(packageJson.exports || {})) {
    if (subpath === '.') {
      assert.equal(target.default, './index.js', 'Root export should continue to target ./index.js.');
      assert.equal(target.types, './index.d.ts', 'Root export should continue to target ./index.d.ts.');
      continue;
    }

    const defaultPath = String(target.default || '').replace(/^\.\//, '');
    const typesPath = String(target.types || '').replace(/^\.\//, '');
    assert.ok(defaultPath, `Export ${subpath} is missing a default entry.`);
    assert.ok(typesPath, `Export ${subpath} is missing a types entry.`);
    assert.ok(packedFiles.has(defaultPath), `Packed Node binding is missing export target ${defaultPath} for ${subpath}.`);
    assert.ok(packedFiles.has(typesPath), `Packed Node binding is missing type target ${typesPath} for ${subpath}.`);
  }

  const tempRoot = mkdtempSync(path.join(os.tmpdir(), 'stateset-node-pack-'));
  try {
    const actualPackOutput = runPack(tempRoot);
    assert.ok(
      Array.isArray(actualPackOutput) && actualPackOutput.length > 0,
      'npm pack should emit a package descriptor when creating a tarball.',
    );
    const tarballPath = path.join(tempRoot, actualPackOutput[0].filename);
    const unpackDir = path.join(tempRoot, 'unpacked');
    mkdirSync(unpackDir, { recursive: true });
    unpackTarball(tarballPath, unpackDir);
    await verifyPackedImports(path.join(unpackDir, 'package'));
  } finally {
    rmSync(tempRoot, { recursive: true, force: true });
  }

  console.log(
    `Node binding package shape is valid for ${pkg.id} with ${pkg.entryCount} packed entries.`,
  );
}

await main();
