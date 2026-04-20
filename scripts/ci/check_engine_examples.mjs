#!/usr/bin/env node

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '../..');
const jsOnly = process.argv.includes('--js-only');
const pythonOnly = process.argv.includes('--python-only');
const DOCS_WITH_REPO_PATHS = [
  'docs/src/ai-agents.md',
  'docs/src/api/go.md',
  'docs/src/api/node.md',
  'docs/src/api/php.md',
  'docs/src/api/python.md',
  'docs/src/api/rust.md',
  'docs/src/api/wasm.md',
  'docs/src/examples.md',
  'examples/README.md',
  'examples/daemon-guide.md',
];

if (jsOnly && pythonOnly) {
  throw new Error('check_engine_examples.mjs accepts at most one of --js-only or --python-only.');
}

function validateDocumentedRepoPaths() {
  const missingPaths = [];

  for (const relativeDocPath of DOCS_WITH_REPO_PATHS) {
    const content = fs.readFileSync(path.join(rootDir, relativeDocPath), 'utf8');
    const documentedPaths = new Set();
    for (const match of content.matchAll(/`((?:examples|bindings|cli|crates)\/[^`]+)`/g)) {
      const documentedPath = match[1];
      if (!documentedPath.includes('*')) {
        documentedPaths.add(documentedPath);
      }
    }

    for (const documentedPath of documentedPaths) {
      if (!fs.existsSync(path.join(rootDir, documentedPath))) {
        missingPaths.push({ doc: relativeDocPath, path: documentedPath });
      }
    }
  }

  assert.equal(
    missingPaths.length,
    0,
    `Documented repo paths must exist.\n${missingPaths
      .map((entry) => `${entry.doc} references missing path ${entry.path}`)
      .join('\n')}`,
  );
}

function runNodeExample(relativeExamplePath) {
  const examplePath = path.join(rootDir, relativeExamplePath);
  const run = spawnSync(process.execPath, [examplePath], {
    cwd: rootDir,
    encoding: 'utf8',
    env: {
      ...process.env,
      STATESET_TOOLKIT_OUTPUT: 'json',
    },
  });
  assert.equal(
    run.status,
    0,
    `Node example ${relativeExamplePath} should succeed.\n${run.stderr || run.stdout}`,
  );
  try {
    return JSON.parse(run.stdout.trim());
  } catch (error) {
    throw new Error(
      `Node example ${relativeExamplePath} should emit JSON summary output.\n${run.stdout}\n${error}`,
    );
  }
}

function resolvePythonBin() {
  if (process.env.STATESET_PYTHON_BIN) {
    return process.env.STATESET_PYTHON_BIN;
  }

  const venvPython = path.join(rootDir, 'bindings/python/.venv/bin/python');
  if (fs.existsSync(venvPython)) {
    return venvPython;
  }

  for (const candidate of ['python3', 'python']) {
    const check = spawnSync(candidate, ['--version'], { encoding: 'utf8' });
    if (check.status === 0) {
      return candidate;
    }
  }

  throw new Error(
    'Python interpreter not found. Set STATESET_PYTHON_BIN or create bindings/python/.venv.',
  );
}

function runPythonExample(pythonBin, relativeExamplePath) {
  const examplePath = path.join(rootDir, relativeExamplePath);
  const run = spawnSync(pythonBin, [examplePath], {
    cwd: rootDir,
    encoding: 'utf8',
    env: {
      ...process.env,
      STATESET_TOOLKIT_OUTPUT: 'json',
    },
  });
  assert.equal(
    run.status,
    0,
    `Python example ${relativeExamplePath} should succeed.\n${run.stderr || run.stdout}`,
  );
  try {
    return JSON.parse(run.stdout.trim());
  } catch (error) {
    throw new Error(
      `Python example ${relativeExamplePath} should emit JSON summary output.\n${run.stdout}\n${error}`,
    );
  }
}

validateDocumentedRepoPaths();

if (!pythonOnly) {
  const openai = runNodeExample('examples/agents/openai-embedded-toolkit.mjs');
  assert.ok(openai.toolCount > 0, 'OpenAI toolkit demo should export at least one tool.');
  assert.equal(
    openai.status,
    'success',
    'OpenAI toolkit demo should execute list_customers successfully.',
  );
  assert.equal(
    openai.outputMessageType,
    'function_call_output',
    'OpenAI toolkit demo should emit an OpenAI-compatible output message.',
  );

  const custom = runNodeExample('examples/agents/custom-framework-adapter.mjs');
  assert.ok(
    custom.descriptorCount >= 3,
    'Custom framework demo should expose at least three descriptors.',
  );
  assert.ok(
    custom.registryKeys.includes('list_customers'),
    'Custom framework demo should expose a callable list_customers descriptor.',
  );
  assert.equal(
    custom.status,
    'success',
    'Custom framework descriptor demo should execute list_customers successfully.',
  );

  const adapters = runNodeExample('examples/agents/framework-adapters.mjs');
  assert.ok(
    adapters.vercelToolKeys.includes('list_customers'),
    'Framework adapter demo should expose list_customers for Vercel AI.',
  );
  assert.ok(
    adapters.langChainToolCount >= 1,
    'Framework adapter demo should expose at least one LangChain tool.',
  );
  assert.equal(
    adapters.status,
    'success',
    'Framework adapter demo should execute list_customers successfully.',
  );
}

if (!jsOnly) {
  const pythonBin = resolvePythonBin();
  const pythonAgentToolkit = runPythonExample(pythonBin, 'examples/python/agent_toolkit.py');
  assert.ok(
    pythonAgentToolkit.toolCount >= 1,
    'Python agent toolkit example should expose at least one OpenAI tool.',
  );
  assert.equal(
    pythonAgentToolkit.status,
    'success',
    'Python agent toolkit example should execute list_customers successfully.',
  );
  assert.equal(
    pythonAgentToolkit.outputMessageType,
    'function_call_output',
    'Python agent toolkit example should emit an OpenAI-compatible output message.',
  );

  const pythonOpenAI = runPythonExample(pythonBin, 'examples/python/openai_tools.py');
  assert.ok(
    pythonOpenAI.tools.includes('list_customers'),
    'Python OpenAI helper example should expose list_customers.',
  );
  assert.equal(
    pythonOpenAI.status,
    'success',
    'Python OpenAI helper example should execute list_customers successfully.',
  );
  assert.equal(
    pythonOpenAI.batchStatus,
    'success',
    'Python OpenAI helper example should execute batched calls successfully.',
  );

  const pythonGeneric = runPythonExample(pythonBin, 'examples/python/generic_tools.py');
  assert.ok(
    pythonGeneric.descriptors.includes('list_customers'),
    'Python generic helper example should expose list_customers descriptors.',
  );
  assert.ok(
    pythonGeneric.registryKeys.includes('list_customers'),
    'Python generic helper example should expose a list_customers registry entry.',
  );
  assert.equal(
    pythonGeneric.status,
    'success',
    'Python generic helper example should execute list_customers successfully.',
  );
  assert.equal(
    pythonGeneric.batchStatus,
    'success',
    'Python generic helper example should execute batched calls successfully.',
  );

  const pythonLangChain = runPythonExample(pythonBin, 'examples/python/langchain_tools.py');
  assert.ok(
    pythonLangChain.tools.includes('list_customers'),
    'Python LangChain example should expose list_customers.',
  );
  assert.equal(
    pythonLangChain.status,
    'success',
    'Python LangChain example should execute list_customers successfully.',
  );

  const pythonCrewAI = runPythonExample(pythonBin, 'examples/python/crewai_tools.py');
  assert.ok(
    pythonCrewAI.tools.includes('count_customers'),
    'Python CrewAI example should expose count_customers.',
  );
  assert.equal(
    pythonCrewAI.status,
    'success',
    'Python CrewAI example should execute count_customers successfully.',
  );
  assert.equal(
    pythonCrewAI.writeStatus,
    'preview',
    'Python CrewAI example should keep write tools in preview mode by default.',
  );

  const pythonAutoGen = runPythonExample(pythonBin, 'examples/python/autogen_tools.py');
  assert.ok(
    pythonAutoGen.tools.includes('get_sales_summary'),
    'Python AutoGen example should expose get_sales_summary.',
  );
  assert.equal(
    pythonAutoGen.status,
    'success',
    'Python AutoGen example should execute get_sales_summary successfully.',
  );
  assert.equal(
    pythonAutoGen.secondaryStatus,
    'success',
    'Python AutoGen example should execute the secondary tool successfully.',
  );

  const pythonAdapters = runPythonExample(pythonBin, 'examples/python/framework_adapters.py');
  assert.ok(
    pythonAdapters.registryKeys.includes('list_customers'),
    'Python framework adapter example should expose a callable list_customers registry entry.',
  );
  assert.equal(
    pythonAdapters.langchainTool,
    'list_customers',
    'Python framework adapter example should expose the LangChain list_customers tool.',
  );
  assert.equal(
    pythonAdapters.crewaiTool,
    'count_customers',
    'Python framework adapter example should expose the CrewAI count_customers tool.',
  );
  assert.equal(
    pythonAdapters.autogenTool,
    'get_sales_summary',
    'Python framework adapter example should expose the AutoGen get_sales_summary tool.',
  );
  assert.equal(
    pythonAdapters.status,
    'success',
    'Python framework adapter example should execute the callable registry successfully.',
  );
}

if (jsOnly) {
  console.log('Embedded engine examples passed for JS surfaces.');
} else if (pythonOnly) {
  console.log('Embedded engine examples passed for Python surfaces.');
} else {
  console.log('Embedded engine examples passed for JS and Python surfaces.');
}
