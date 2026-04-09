#!/usr/bin/env node

import { execFile } from 'node:child_process';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';

const execFileAsync = promisify(execFile);

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '../..');
const jsonOutputPath = path.join(rootDir, 'artifacts/compatibility/workspace-inventory.json');
const markdownOutputPath = path.join(rootDir, 'docs/src/appendix/workspace-inventory.md');
const checkMode = process.argv.includes('--check');

const PRODUCT_GRAPH_EXCLUDES = new Set([
  'stateset-benches',
  'stateset-integration-tests',
  'stateset-test-utils',
]);

function compareStrings(left, right) {
  return left.replace(/\r\n/g, '\n') === right.replace(/\r\n/g, '\n');
}

function renderMarkdownTable(headers, rows) {
  const headerRow = `| ${headers.join(' | ')} |`;
  const dividerRow = `| ${headers.map(() => '---').join(' | ')} |`;
  const bodyRows = rows.map((row) => `| ${row.join(' | ')} |`);
  return [headerRow, dividerRow, ...bodyRows].join('\n');
}

function packageKind(relativeManifestPath) {
  if (relativeManifestPath.startsWith('crates/')) {
    return 'crate';
  }
  if (relativeManifestPath.startsWith('bindings/')) {
    return 'binding';
  }
  return 'other';
}

function escapeCodeList(values) {
  return values.map((value) => `\`${value}\``).join(', ');
}

function parseCargoManifest(text) {
  const nameMatch = text.match(/^\s*name\s*=\s*"([^"]+)"/m);
  const descriptionMatch = text.match(/^\s*description\s*=\s*"([^"]+)"/m);
  return {
    name: nameMatch?.[1] ?? null,
    description: descriptionMatch?.[1] ?? null,
  };
}

async function readJson(filePath) {
  return JSON.parse(await readFile(filePath, 'utf8'));
}

async function runCargoMetadata() {
  const { stdout } = await execFileAsync('cargo', ['metadata', '--no-deps', '--format-version', '1'], {
    cwd: rootDir,
    maxBuffer: 64 * 1024 * 1024,
  });
  return JSON.parse(stdout);
}

function normalizeWorkspacePackages(metadata) {
  return metadata.packages
    .map((pkg) => {
      const relativeManifestPath = path.relative(rootDir, pkg.manifest_path);
      return {
        name: pkg.name,
        description: pkg.description ?? '',
        manifestPath: relativeManifestPath,
        packageKind: packageKind(relativeManifestPath),
        dependencies: pkg.dependencies,
      };
    })
    .sort((left, right) => left.manifestPath.localeCompare(right.manifestPath));
}

function buildDependencyGraph(packages, excludeNames = new Set()) {
  const includedPackages = packages.filter((pkg) => !excludeNames.has(pkg.name));
  const internalNames = new Set(includedPackages.map((pkg) => pkg.name));
  const forward = new Map();
  const reverse = new Map();

  for (const pkg of includedPackages) {
    const directDeps = [...new Set(pkg.dependencies
      .filter((dependency) => dependency.path && dependency.kind !== 'dev' && internalNames.has(dependency.name))
      .map((dependency) => dependency.name))]
      .sort((left, right) => left.localeCompare(right));

    forward.set(pkg.name, directDeps);
    for (const depName of directDeps) {
      if (!reverse.has(depName)) {
        reverse.set(depName, new Set());
      }
      reverse.get(depName).add(pkg.name);
    }
  }

  const remaining = new Map(
    [...forward.entries()].map(([name, deps]) => [name, new Set(deps)]),
  );
  const layers = [];

  while (remaining.size > 0) {
    const layer = [...remaining.entries()]
      .filter(([, deps]) => deps.size === 0)
      .map(([name]) => name)
      .sort((left, right) => left.localeCompare(right));

    if (layer.length === 0) {
      const cycleFallback = [...remaining.keys()].sort((left, right) => left.localeCompare(right))[0];
      layers.push([cycleFallback]);
      remaining.delete(cycleFallback);
      for (const deps of remaining.values()) {
        deps.delete(cycleFallback);
      }
      continue;
    }

    layers.push(layer);
    for (const name of layer) {
      remaining.delete(name);
    }
    for (const deps of remaining.values()) {
      for (const name of layer) {
        deps.delete(name);
      }
    }
  }

  const topFanIn = [...reverse.entries()]
    .map(([name, dependents]) => ({
      name,
      dependentCount: dependents.size,
      dependents: [...dependents].sort((left, right) => left.localeCompare(right)),
    }))
    .sort((left, right) => {
      if (right.dependentCount !== left.dependentCount) {
        return right.dependentCount - left.dependentCount;
      }
      return left.name.localeCompare(right.name);
    });

  return {
    forward: Object.fromEntries([...forward.entries()].sort((left, right) => left[0].localeCompare(right[0]))),
    layers,
    topFanIn,
  };
}

async function getExcludedBindingManifests(workspacePackageNames) {
  const bindingsDir = path.join(rootDir, 'bindings');
  const entries = await readdir(bindingsDir, { withFileTypes: true });
  const excluded = [];

  for (const entry of entries) {
    if (!entry.isDirectory()) {
      continue;
    }

    const manifestPath = path.join(bindingsDir, entry.name, 'Cargo.toml');
    try {
      const content = await readFile(manifestPath, 'utf8');
      const parsed = parseCargoManifest(content);
      if (parsed.name && !workspacePackageNames.has(parsed.name)) {
        excluded.push({
          directory: `bindings/${entry.name}`,
          name: parsed.name,
          description: parsed.description ?? '',
        });
      }
    } catch {
      // Skip directories without a Cargo manifest.
    }
  }

  return excluded.sort((left, right) => left.directory.localeCompare(right.directory));
}

async function getBindingTopology(bindingPackages) {
  const results = [];

  for (const pkg of bindingPackages) {
    const bindingDir = path.dirname(pkg.manifestPath);
    let publishedPackage = '';

    try {
      const packageJson = await readJson(path.join(rootDir, bindingDir, 'package.json'));
      publishedPackage = packageJson.name ?? '';
    } catch {
      publishedPackage = '';
    }

    const internalDeps = [...new Set(pkg.dependencies
      .filter((dependency) => dependency.path && dependency.kind !== 'dev')
      .map((dependency) => dependency.name))]
      .sort((left, right) => left.localeCompare(right));

    results.push({
      bindingDir,
      cargoPackage: pkg.name,
      publishedPackage,
      internalDeps,
    });
  }

  return results.sort((left, right) => left.bindingDir.localeCompare(right.bindingDir));
}

async function countFilesRecursive(dirPath) {
  let total = 0;
  const entries = await readdir(dirPath, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(dirPath, entry.name);
    if (entry.isDirectory()) {
      total += await countFilesRecursive(fullPath);
    } else if (entry.isFile()) {
      total += 1;
    }
  }
  return total;
}

async function getCliInventory() {
  const pkg = await readJson(path.join(rootDir, 'cli/package.json'));
  const srcDir = path.join(rootDir, 'cli/src');
  const topLevelEntries = await readdir(srcDir, { withFileTypes: true });
  const topLevelSourceCounts = [];

  for (const entry of topLevelEntries) {
    const fullPath = path.join(srcDir, entry.name);
    const count = entry.isDirectory() ? await countFilesRecursive(fullPath) : 1;
    topLevelSourceCounts.push({ name: entry.name, count });
  }

  topLevelSourceCounts.sort((left, right) => {
    if (right.count !== left.count) {
      return right.count - left.count;
    }
    return left.name.localeCompare(right.name);
  });

  return {
    binaryCount: Object.keys(pkg.bin ?? {}).length,
    dependencyCount: Object.keys(pkg.dependencies ?? {}).length,
    optionalDependencyCount: Object.keys(pkg.optionalDependencies ?? {}).length,
    topLevelSourceCounts,
    toolModuleCount: await countFilesRecursive(path.join(srcDir, 'tools')),
    a2aModuleCount: await countFilesRecursive(path.join(srcDir, 'a2a')),
  };
}

async function getAdminInventory() {
  const pkg = await readJson(path.join(rootDir, 'admin/package.json'));
  return {
    dependencyCount: Object.keys(pkg.dependencies ?? {}).length,
    devDependencyCount: Object.keys(pkg.devDependencies ?? {}).length,
    embeddedBindingSource: pkg.dependencies?.['@stateset/embedded'] ?? '',
  };
}

function renderMarkdownInventory(inventory) {
  const summaryRows = [
    ['Workspace version', `\`${inventory.workspace.version}\``],
    ['Workspace members', String(inventory.workspace.memberCount)],
    ['Default members', String(inventory.workspace.defaultMemberCount)],
    ['Rust crates in workspace', String(inventory.workspace.rustCrateCount)],
    ['Binding crates in workspace', String(inventory.workspace.bindingCrateCount)],
    ['Excluded local binding manifests', String(inventory.excludedBindings.length)],
    ['CLI binaries', String(inventory.cli.binaryCount)],
    ['CLI optional dependencies', String(inventory.cli.optionalDependencyCount)],
    ['Admin local embedded binding', `\`${inventory.admin.embeddedBindingSource}\``],
  ];

  const layerRows = inventory.productGraph.layers.map((packages, index) => [
    `L${index + 1}`,
    escapeCodeList(packages),
  ]);

  const fanInRows = inventory.productGraph.topFanIn.slice(0, 10).map((entry) => [
    `\`${entry.name}\``,
    String(entry.dependentCount),
  ]);

  const bindingRows = inventory.bindingTopology.map((entry) => [
    `\`${entry.bindingDir}\``,
    `\`${entry.cargoPackage}\``,
    entry.publishedPackage ? `\`${entry.publishedPackage}\`` : '—',
    entry.internalDeps.length > 0 ? escapeCodeList(entry.internalDeps) : '—',
  ]);

  const excludedRows = inventory.excludedBindings.map((entry) => [
    `\`${entry.directory}\``,
    `\`${entry.name}\``,
    entry.description || '—',
  ]);

  const cliSummaryRows = [
    ['Top-level source groups', String(inventory.cli.topLevelSourceCounts.length)],
    ['Tool modules', String(inventory.cli.toolModuleCount)],
    ['A2A modules', String(inventory.cli.a2aModuleCount)],
    ['JS dependencies', String(inventory.cli.dependencyCount)],
    ['Optional integrations', String(inventory.cli.optionalDependencyCount)],
  ];

  const cliTopLevelRows = inventory.cli.topLevelSourceCounts.map((entry) => [
    `\`${entry.name}\``,
    String(entry.count),
  ]);

  return `# Workspace Inventory

This page is generated from the local workspace manifests and package metadata.
Do not edit it by hand. Regenerate it with:

\`\`\`bash
node ./scripts/ci/generate_workspace_inventory.mjs
\`\`\`

Machine-readable output lives at \`artifacts/compatibility/workspace-inventory.json\`.

## Summary

${renderMarkdownTable(['Metric', 'Value'], summaryRows)}

## Product Graph Layers

These layers are computed from direct internal dependencies after excluding
test-only support crates ('stateset-benches', 'stateset-integration-tests',
and 'stateset-test-utils') so the runtime/product graph is easier to read.

${renderMarkdownTable(['Layer', 'Packages'], layerRows)}

## Highest Fan-In Crates

${renderMarkdownTable(['Package', 'Direct dependents'], fanInRows)}

## Binding Topology

${renderMarkdownTable(['Binding', 'Cargo package', 'Published package', 'Direct internal deps'], bindingRows)}

## Excluded Local Binding Manifests

These binding crates exist in-repo but are intentionally excluded from default
workspace membership because they require host runtimes or headers.

${renderMarkdownTable(['Directory', 'Cargo package', 'Description'], excludedRows)}

## CLI Surface

${renderMarkdownTable(['Metric', 'Value'], cliSummaryRows)}

## CLI Top-Level Source Groups

${renderMarkdownTable(['Group', 'Files'], cliTopLevelRows)}
`;
}

async function verifyOutput(filePath, expectedContent) {
  const relativePath = path.relative(rootDir, filePath);

  try {
    const actualContent = await readFile(filePath, 'utf8');
    if (!compareStrings(actualContent, expectedContent)) {
      console.error(
        `::error file=${relativePath}::Generated workspace inventory is out of date. Run 'node ./scripts/ci/generate_workspace_inventory.mjs'.`,
      );
      return false;
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : 'unknown error';
    console.error(
      `::error file=${relativePath}::Unable to read generated workspace inventory output (${message}). Run 'node ./scripts/ci/generate_workspace_inventory.mjs'.`,
    );
    return false;
  }

  return true;
}

async function buildInventory() {
  const metadata = await runCargoMetadata();
  const workspacePackages = normalizeWorkspacePackages(metadata);
  const workspacePackageNames = new Set(workspacePackages.map((pkg) => pkg.name));
  const rustCrates = workspacePackages.filter((pkg) => pkg.packageKind === 'crate');
  const bindingCrates = workspacePackages.filter((pkg) => pkg.packageKind === 'binding');
  const productGraph = buildDependencyGraph(workspacePackages, PRODUCT_GRAPH_EXCLUDES);
  const bindingTopology = await getBindingTopology(bindingCrates);
  const cli = await getCliInventory();
  const admin = await getAdminInventory();

  return {
    source: {
      cargoMetadata: 'cargo metadata --no-deps --format-version 1',
      cliPackage: 'cli/package.json',
      adminPackage: 'admin/package.json',
    },
    workspace: {
      version: metadata.workspace_root ? metadata.packages[0]?.version ?? '' : '',
      memberCount: metadata.workspace_members.length,
      defaultMemberCount: metadata.workspace_default_members.length,
      rustCrateCount: rustCrates.length,
      bindingCrateCount: bindingCrates.length,
      members: workspacePackages.map((pkg) => ({
        name: pkg.name,
        manifestPath: pkg.manifestPath,
        packageKind: pkg.packageKind,
      })),
    },
    productGraph,
    bindingTopology,
    excludedBindings: await getExcludedBindingManifests(workspacePackageNames),
    cli,
    admin,
  };
}

async function main() {
  const inventory = await buildInventory();
  const jsonContent = `${JSON.stringify(inventory, null, 2)}\n`;
  const markdownContent = renderMarkdownInventory(inventory);

  if (checkMode) {
    const ok = await Promise.all([
      verifyOutput(jsonOutputPath, jsonContent),
      verifyOutput(markdownOutputPath, markdownContent),
    ]);

    if (!ok.every(Boolean)) {
      process.exit(1);
    }

    console.log(
      `Workspace inventory is up to date (${inventory.workspace.memberCount} workspace members, ${inventory.cli.binaryCount} CLI binaries).`,
    );
    return;
  }

  await mkdir(path.dirname(jsonOutputPath), { recursive: true });
  await mkdir(path.dirname(markdownOutputPath), { recursive: true });
  await writeFile(jsonOutputPath, jsonContent, 'utf8');
  await writeFile(markdownOutputPath, markdownContent, 'utf8');

  console.log(
    `Generated workspace inventory (${inventory.workspace.memberCount} workspace members, ${inventory.cli.binaryCount} CLI binaries).`,
  );
}

await main();
