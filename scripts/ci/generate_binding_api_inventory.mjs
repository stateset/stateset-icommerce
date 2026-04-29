#!/usr/bin/env node

import { mkdir, readdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '../..');
const jsonOutputPath = path.join(rootDir, 'artifacts/compatibility/binding-api-inventory.json');
const markdownOutputPath = path.join(rootDir, 'docs/src/appendix/binding-api-inventory.md');
const checkMode = process.argv.includes('--check');

function compareStrings(left, right) {
  return left.replace(/\r\n/g, '\n') === right.replace(/\r\n/g, '\n');
}

function renderMarkdownTable(headers, rows) {
  const headerRow = `| ${headers.join(' | ')} |`;
  const dividerRow = `| ${headers.map(() => '---').join(' | ')} |`;
  const bodyRows = rows.map((row) => `| ${row.join(' | ')} |`);
  return [headerRow, dividerRow, ...bodyRows].join('\n');
}

async function readText(relativePath) {
  return readFile(path.join(rootDir, relativePath), 'utf8');
}

async function readJson(relativePath) {
  return JSON.parse(await readText(relativePath));
}

function extractOne(text, regex) {
  const match = text.match(regex);
  return match?.[1] ?? null;
}

function extractAll(text, regex) {
  return [...text.matchAll(regex)].map((match) => match[1]);
}

function parseTomlString(text, key) {
  return extractOne(text, new RegExp(`^${key}\\s*=\\s*"([^"]+)"`, 'm'));
}

function parseTomlListSectionKeys(text, sectionName) {
  const lines = text.split('\n');
  const header = `[${sectionName}]`;
  const startIndex = lines.findIndex((line) => line.trim() === header);
  if (startIndex < 0) {
    return [];
  }

  const sectionLines = [];
  for (const line of lines.slice(startIndex + 1)) {
    if (line.startsWith('[')) {
      break;
    }
    sectionLines.push(line);
  }

  return extractAll(sectionLines.join('\n'), /^([A-Za-z0-9_-]+)\s*=/gm).sort((left, right) =>
    left.localeCompare(right),
  );
}

function parseStringListAssignment(text, variableName) {
  const match = text.match(
    new RegExp(`${variableName}\\s*=\\s*\\[([\\s\\S]*?)\\]`, 'm'),
  );
  if (!match) {
    return [];
  }

  return [...match[1].matchAll(/"([^"]+)"|'([^']+)'/g)]
    .map((entry) => entry[1] || entry[2])
    .sort((left, right) => left.localeCompare(right));
}

function sortUnique(values) {
  return [...new Set(values.filter(Boolean))].sort((left, right) => left.localeCompare(right));
}

function collectTypeSections(text, declarationPattern, { kindIndex = 1, nameIndex = 2 } = {}) {
  const matches = [];
  let match;
  while ((match = declarationPattern.exec(text)) !== null) {
    matches.push({
      index: match.index,
      kind: match[kindIndex],
      name: match[nameIndex],
    });
  }

  return matches.map((entry, index) => ({
    ...entry,
    body: text.slice(entry.index, matches[index + 1]?.index ?? text.length),
  }));
}

function renderSingleColumnRows(values) {
  return values.map((value) => [`\`${value}\``]);
}

async function buildNodeBindingInventory() {
  const manifestPath = 'bindings/node/package.json';
  const readmePath = 'bindings/node/README.md';
  const packageJson = await readJson(manifestPath);

  const entrypoints = Object.entries(packageJson.exports || {})
    .map(([subpath, target]) => ({
      subpath,
      runtimeEntry: target.default || null,
      typesEntry: target.types || null,
    }))
    .sort((left, right) => left.subpath.localeCompare(right.subpath));

  return {
    language: 'Node.js',
    ecosystem: 'npm',
    directory: 'bindings/node',
    manifestPath,
    readmePath,
    coverageLevel: 'detailed',
    packageName: packageJson.name,
    version: packageJson.version,
    runtime: 'napi-rs',
    entrypointCount: entrypoints.length,
    packedFilePatternCount: Array.isArray(packageJson.files) ? packageJson.files.length : 0,
    peerDependencies: Object.keys(packageJson.peerDependencies || {}).sort(),
    entrypoints,
  };
}

async function buildPythonBindingInventory() {
  const manifestPath = 'bindings/python/pyproject.toml';
  const readmePath = 'bindings/python/README.md';
  const initPath = 'bindings/python/python/stateset_embedded/__init__.py';
  const pyproject = await readText(manifestPath);
  const initModule = await readText(initPath);
  const packageEntries = await readdir(
    path.join(rootDir, 'bindings/python/python/stateset_embedded'),
    { withFileTypes: true },
  );

  const publicSymbols = parseStringListAssignment(initModule, '__all__');
  const helperModules = packageEntries
    .filter((entry) => entry.isFile())
    .map((entry) => entry.name)
    .filter((name) => name.endsWith('.py') && name !== '__init__.py')
    .map((name) => name.replace(/\.py$/, ''))
    .sort((left, right) => left.localeCompare(right));

  return {
    language: 'Python',
    ecosystem: 'PyPI',
    directory: 'bindings/python',
    manifestPath,
    readmePath,
    coverageLevel: 'detailed',
    packageName: parseTomlString(pyproject, 'name'),
    version: parseTomlString(pyproject, 'version'),
    requiresPython: parseTomlString(pyproject, 'requires-python'),
    optionalDependencyGroups: parseTomlListSectionKeys(
      pyproject,
      'project.optional-dependencies',
    ),
    moduleCount: helperModules.length + 1,
    publicSymbolCount: publicSymbols.length,
    helperModules,
    nativeExtensions: [],
    publicSymbols,
  };
}

async function buildWasmBindingInventory() {
  const manifestPath = 'bindings/wasm/package.json';
  const readmePath = 'bindings/wasm/README.md';
  const packageJson = await readJson(manifestPath);

  return {
    language: 'WASM',
    ecosystem: 'npm',
    directory: 'bindings/wasm',
    manifestPath,
    readmePath,
    coverageLevel: 'package-manifest',
    packageName: packageJson.name,
    version: packageJson.version,
    entrypoints: {
      main: packageJson.main || null,
      module: packageJson.module || null,
      types: packageJson.types || null,
    },
  };
}

async function buildGoBindingInventory() {
  const manifestPath = 'bindings/go/stateset/go.mod';
  const readmePath = 'bindings/go/README.md';
  const sourceFiles = [
    'bindings/go/stateset/stateset.go',
    'bindings/go/stateset/apis.go',
    'bindings/go/stateset/models.go',
  ];
  const [goMod, ...sources] = await Promise.all([
    readText(manifestPath),
    ...sourceFiles.map((relativePath) => readText(relativePath)),
  ]);
  const combinedSource = sources.join('\n');

  const exportedTypes = sortUnique(extractAll(combinedSource, /^type\s+([A-Z]\w*)\b/gm));
  const apiTypes = exportedTypes.filter((name) => name.endsWith('API'));
  const topLevelFunctions = sortUnique(extractAll(combinedSource, /^func\s+([A-Z]\w*)\s*\(/gm));
  const methodMatches = [...combinedSource.matchAll(
    /^func\s+\(\s*\w+\s+\*?([A-Z]\w*)\s*\)\s+([A-Z]\w*)\s*\(/gm,
  )].map((match) => ({ receiver: match[1], name: match[2] }));
  const rootAccessors = sortUnique(
    methodMatches
      .filter((entry) => entry.receiver === 'Commerce' && entry.name !== 'Close')
      .map((entry) => entry.name),
  );
  const apiMethods = methodMatches
    .filter((entry) => entry.receiver.endsWith('API'))
    .sort(
      (left, right) =>
        left.receiver.localeCompare(right.receiver) || left.name.localeCompare(right.name),
    );

  return {
    language: 'Go',
    ecosystem: 'Go modules',
    directory: 'bindings/go',
    manifestPath,
    readmePath,
    coverageLevel: 'detailed',
    packageName: extractOne(goMod, /^module\s+(.+)$/m),
    version: null,
    sourceFiles,
    exportedTypeCount: exportedTypes.length,
    apiTypeCount: apiTypes.length,
    rootAccessorCount: rootAccessors.length,
    apiMethodCount: apiMethods.length,
    exportedTypes,
    apiTypes,
    topLevelFunctions,
    rootAccessors,
    apiMethods,
  };
}

async function buildPhpBindingInventory() {
  const manifestPath = 'bindings/php/composer.json';
  const readmePath = 'bindings/php/README.md';
  const composer = await readJson(manifestPath);

  return {
    language: 'PHP',
    ecosystem: 'Composer',
    directory: 'bindings/php',
    manifestPath,
    readmePath,
    coverageLevel: 'package-manifest',
    packageName: composer.name,
    version: composer.version || null,
    requiresPhp: composer.require?.php || null,
    autoloadFiles: composer.autoload?.files || [],
    suggestedExtensions: Object.keys(composer.suggest || {}).sort(),
  };
}

async function buildRubyBindingInventory() {
  const manifestPath = 'bindings/ruby/stateset_embedded.gemspec';
  const readmePath = 'bindings/ruby/README.md';
  const gemspec = await readText(manifestPath);

  return {
    language: 'Ruby',
    ecosystem: 'RubyGems',
    directory: 'bindings/ruby',
    manifestPath,
    readmePath,
    coverageLevel: 'package-manifest',
    packageName: extractOne(gemspec, /s\.name\s*=\s*'([^']+)'/),
    version: extractOne(gemspec, /s\.version\s*=\s*'([^']+)'/),
    requiredVersion: extractOne(gemspec, /s\.required_ruby_version\s*=\s*'([^']+)'/),
    nativeExtensions: parseStringListAssignment(gemspec, 's.extensions'),
  };
}

async function buildSwiftBindingInventory() {
  const manifestPath = 'bindings/swift/Package.swift';
  const readmePath = 'bindings/swift/README.md';
  const sourceFiles = [
    'bindings/swift/Sources/StateSet/StateSetCommerce.swift',
    'bindings/swift/Sources/StateSet/APIs.swift',
    'bindings/swift/Sources/StateSet/Models.swift',
  ];
  const [packageSwift, commerceSwift, apiSwift, modelsSwift] = await Promise.all([
    readText(manifestPath),
    ...sourceFiles.map((relativePath) => readText(relativePath)),
  ]);
  const typeSections = collectTypeSections(
    [commerceSwift, apiSwift, modelsSwift].join('\n'),
    /^\s*public\s+(?:final\s+)?(class|struct|enum)\s+([A-Z]\w*)\b/gm,
  );
  const publicTypes = sortUnique(typeSections.map((section) => section.name));
  const apiTypes = publicTypes.filter((name) => name.endsWith('API'));
  const facadeProperties = [...commerceSwift.matchAll(
    /^\s*public(?:\s+private\(set\))?\s+(?:lazy\s+)?var\s+([a-zA-Z_]\w*)\s*=\s*([A-Z]\w*)\(/gm,
  )]
    .map((match) => ({ name: match[1], type: match[2] }))
    .sort((left, right) => left.name.localeCompare(right.name));
  const apiMethodRows = typeSections
    .filter((section) => section.name.endsWith('API'))
    .flatMap((section) =>
      sortUnique(extractAll(section.body, /^\s*public\s+func\s+([a-zA-Z_]\w*)\s*\(/gm)).map(
        (method) => ({ type: section.name, method }),
      ),
    )
    .sort((left, right) => left.type.localeCompare(right.type) || left.method.localeCompare(right.method));

  return {
    language: 'Swift',
    ecosystem: 'SwiftPM',
    directory: 'bindings/swift',
    manifestPath,
    readmePath,
    coverageLevel: 'detailed',
    packageName: extractOne(packageSwift, /Package\(\s*name:\s*"([^"]+)"/),
    version: null,
    sourceFiles,
    products: extractAll(packageSwift, /\.library\(\s*name:\s*"([^"]+)"/g).sort(),
    targets: extractAll(packageSwift, /\.(?:target|testTarget)\(\s*name:\s*"([^"]+)"/g).sort(),
    publicTypeCount: publicTypes.length,
    apiTypeCount: apiTypes.length,
    facadePropertyCount: facadeProperties.length,
    apiMethodCount: apiMethodRows.length,
    publicTypes,
    apiTypes,
    facadeProperties,
    apiMethods: apiMethodRows,
  };
}

async function buildJavaBindingInventory() {
  const manifestPath = 'bindings/java/java/build.gradle';
  const readmePath = 'bindings/java/README.md';
  const buildGradle = await readText(manifestPath);

  const group = extractOne(buildGradle, /^group\s*=\s*'([^']+)'/m);
  const artifactId = extractOne(buildGradle, /artifactId\s*=\s*'([^']+)'/);

  return {
    language: 'Java',
    ecosystem: 'Maven',
    directory: 'bindings/java',
    manifestPath,
    readmePath,
    coverageLevel: 'package-manifest',
    packageName: group && artifactId ? `${group}:${artifactId}` : null,
    version: extractOne(buildGradle, /^version\s*=\s*'([^']+)'/m),
    targetJava: extractOne(
      buildGradle,
      /sourceCompatibility\s*=\s*JavaVersion\.VERSION_([0-9_]+)/,
    )?.replace(/_/g, '.'),
  };
}

async function buildKotlinBindingInventory() {
  const manifestPath = 'bindings/kotlin/kotlin/build.gradle.kts';
  const buildGradle = await readText(manifestPath);

  const group = extractOne(buildGradle, /^group\s*=\s*"([^"]+)"/m);
  const artifactId = extractOne(buildGradle, /artifactId\s*=\s*"([^"]+)"/);

  return {
    language: 'Kotlin',
    ecosystem: 'Maven',
    directory: 'bindings/kotlin',
    manifestPath,
    readmePath: null,
    coverageLevel: 'package-manifest',
    packageName: group && artifactId ? `${group}:${artifactId}` : null,
    version: extractOne(buildGradle, /^version\s*=\s*"([^"]+)"/m),
    targetJava: extractOne(buildGradle, /jvmToolchain\(([0-9]+)\)/),
  };
}

async function buildDotnetBindingInventory() {
  const manifestPath = 'bindings/dotnet/dotnet/StateSet/StateSet.csproj';
  const readmePath = 'bindings/dotnet/README.md';
  const sourceFiles = [
    'bindings/dotnet/dotnet/StateSet/StateSetCommerce.cs',
    'bindings/dotnet/dotnet/StateSet/APIs.cs',
    'bindings/dotnet/dotnet/StateSet/Models.cs',
  ];
  const [csproj, commerceCs, apiCs, modelCs] = await Promise.all([
    readText(manifestPath),
    ...sourceFiles.map((relativePath) => readText(relativePath)),
  ]);
  const typeSections = collectTypeSections(
    [commerceCs, apiCs, modelCs].join('\n'),
    /^\s*public\s+(?:sealed\s+)?(class|record|enum)\s+([A-Z]\w*)\b/gm,
  );
  const publicTypes = sortUnique(typeSections.map((section) => section.name));
  const apiTypes = publicTypes.filter((name) => name.endsWith('Api'));
  const facadeProperties = [...commerceCs.matchAll(
    /^\s*public\s+([A-Z][A-Za-z0-9_<>\[\]\?.,]+)\s+([A-Z]\w*)\s*\{\s*get;\s*\}/gm,
  )]
    .map((match) => ({ type: match[1], name: match[2] }))
    .filter((entry) => entry.type.endsWith('Api'))
    .sort((left, right) => left.name.localeCompare(right.name));
  const apiMethodRows = typeSections
    .filter((section) => section.name.endsWith('Api'))
    .flatMap((section) =>
      sortUnique(extractAll(section.body, /^\s*public\s+(?:static\s+)?(?:[A-Za-z0-9_<>\[\]\?.,]+\s+)?([A-Z]\w*)\s*\(/gm)).map(
        (method) => ({ type: section.name, method }),
      ),
    )
    .sort((left, right) => left.type.localeCompare(right.type) || left.method.localeCompare(right.method));

  return {
    language: '.NET',
    ecosystem: 'NuGet',
    directory: 'bindings/dotnet',
    manifestPath,
    readmePath,
    coverageLevel: 'detailed',
    packageName: extractOne(csproj, /<PackageId>([^<]+)<\/PackageId>/),
    version: extractOne(csproj, /<Version>([^<]+)<\/Version>/),
    sourceFiles,
    targetFrameworks: (extractOne(csproj, /<TargetFrameworks>([^<]+)<\/TargetFrameworks>/) || '')
      .split(';')
      .filter(Boolean),
    publicTypeCount: publicTypes.length,
    apiTypeCount: apiTypes.length,
    facadePropertyCount: facadeProperties.length,
    apiMethodCount: apiMethodRows.length,
    publicTypes,
    apiTypes,
    facadeProperties,
    apiMethods: apiMethodRows,
  };
}

async function buildInventory() {
  const bindings = [
    await buildDotnetBindingInventory(),
    await buildGoBindingInventory(),
    await buildJavaBindingInventory(),
    await buildKotlinBindingInventory(),
    await buildNodeBindingInventory(),
    await buildPhpBindingInventory(),
    await buildPythonBindingInventory(),
    await buildRubyBindingInventory(),
    await buildSwiftBindingInventory(),
    await buildWasmBindingInventory(),
  ].sort((left, right) => left.language.localeCompare(right.language));

  const ecosystemCounts = new Map();
  for (const binding of bindings) {
    ecosystemCounts.set(binding.ecosystem, (ecosystemCounts.get(binding.ecosystem) ?? 0) + 1);
  }

  return {
    source: {
      root: 'bindings',
      generator: 'scripts/ci/generate_binding_api_inventory.mjs',
    },
    totalBindings: bindings.length,
    detailedBindingCount: bindings.filter((binding) => binding.coverageLevel === 'detailed').length,
    ecosystems: [...ecosystemCounts.entries()]
      .sort((left, right) => left[0].localeCompare(right[0]))
      .map(([name, count]) => ({ name, count })),
    bindings,
  };
}

function renderBindingSummary(binding) {
  switch (binding.language) {
    case 'Go':
      return `${binding.apiMethodCount} API methods`;
    case 'Node.js':
      return `${binding.entrypointCount} export entrypoints`;
    case 'Python':
      return `${binding.publicSymbolCount} public symbols`;
    case '.NET':
      return `${binding.apiMethodCount} API methods`;
    case 'Swift':
      return `${binding.apiMethodCount} API methods`;
    default:
      return binding.coverageLevel === 'detailed' ? 'detailed surface' : 'manifest coverage';
  }
}

function renderMarkdownInventory(inventory) {
  const summaryRows = [
    ['Binding packages', String(inventory.totalBindings)],
    ['Detailed surfaces', String(inventory.detailedBindingCount)],
    ['Ecosystems', String(inventory.ecosystems.length)],
  ];

  const ecosystemRows = inventory.ecosystems.map((entry) => [entry.name, String(entry.count)]);
  const bindingRows = inventory.bindings.map((binding) => [
    binding.language,
    binding.ecosystem,
    binding.packageName ? `\`${binding.packageName}\`` : '—',
    binding.version ? `\`${binding.version}\`` : '—',
    `\`${binding.coverageLevel}\``,
    renderBindingSummary(binding),
  ]);

  const nodeBinding = inventory.bindings.find((binding) => binding.language === 'Node.js');
  const pythonBinding = inventory.bindings.find((binding) => binding.language === 'Python');

  const nodeRows = nodeBinding.entrypoints.map((entrypoint) => [
    `\`${entrypoint.subpath}\``,
    entrypoint.runtimeEntry ? `\`${entrypoint.runtimeEntry}\`` : '—',
    entrypoint.typesEntry ? `\`${entrypoint.typesEntry}\`` : '—',
  ]);

  const goBinding = inventory.bindings.find((binding) => binding.language === 'Go');
  const goSummaryRows = [
    ['Exported types', String(goBinding.exportedTypeCount)],
    ['API types', String(goBinding.apiTypeCount)],
    ['Root accessors', String(goBinding.rootAccessorCount)],
    ['API methods', String(goBinding.apiMethodCount)],
  ];
  const goTypeRows = renderSingleColumnRows(goBinding.exportedTypes);
  const goAccessorRows = renderSingleColumnRows(goBinding.rootAccessors);
  const goMethodRows = goBinding.apiMethods.map((entry) => [
    `\`${entry.receiver}\``,
    `\`${entry.name}\``,
  ]);

  const dotnetBinding = inventory.bindings.find((binding) => binding.language === '.NET');
  const dotnetSummaryRows = [
    ['Public types', String(dotnetBinding.publicTypeCount)],
    ['API types', String(dotnetBinding.apiTypeCount)],
    ['Facade properties', String(dotnetBinding.facadePropertyCount)],
    ['API methods', String(dotnetBinding.apiMethodCount)],
    ['Target frameworks', dotnetBinding.targetFrameworks.map((framework) => `\`${framework}\``).join(', ')],
  ];
  const dotnetTypeRows = renderSingleColumnRows(dotnetBinding.publicTypes);
  const dotnetFacadeRows = dotnetBinding.facadeProperties.map((entry) => [
    `\`${entry.name}\``,
    `\`${entry.type}\``,
  ]);
  const dotnetMethodRows = dotnetBinding.apiMethods.map((entry) => [
    `\`${entry.type}\``,
    `\`${entry.method}\``,
  ]);

  const pythonModuleRows = pythonBinding.helperModules.map((moduleName) => [`\`${moduleName}\``]);
  const pythonSymbolRows = pythonBinding.publicSymbols.map((symbol) => [`\`${symbol}\``]);

  const swiftBinding = inventory.bindings.find((binding) => binding.language === 'Swift');
  const swiftSummaryRows = [
    ['Public types', String(swiftBinding.publicTypeCount)],
    ['API types', String(swiftBinding.apiTypeCount)],
    ['Facade properties', String(swiftBinding.facadePropertyCount)],
    ['API methods', String(swiftBinding.apiMethodCount)],
    ['Targets', swiftBinding.targets.map((target) => `\`${target}\``).join(', ')],
  ];
  const swiftTypeRows = renderSingleColumnRows(swiftBinding.publicTypes);
  const swiftFacadeRows = swiftBinding.facadeProperties.map((entry) => [
    `\`${entry.name}\``,
    `\`${entry.type}\``,
  ]);
  const swiftMethodRows = swiftBinding.apiMethods.map((entry) => [
    `\`${entry.type}\``,
    `\`${entry.method}\``,
  ]);

  return `# Binding API Inventory

This page is generated from the language binding manifests and exported package surfaces under \`bindings/\`.
Do not edit it by hand. Regenerate it with:

\`\`\`bash
node ./scripts/ci/generate_binding_api_inventory.mjs
\`\`\`

Machine-readable output lives at \`artifacts/compatibility/binding-api-inventory.json\`.

## Summary

${renderMarkdownTable(['Metric', 'Value'], summaryRows)}

## Ecosystem Counts

${renderMarkdownTable(['Ecosystem', 'Bindings'], ecosystemRows)}

## Binding Overview

${renderMarkdownTable(['Language', 'Ecosystem', 'Package', 'Version', 'Coverage', 'Summary'], bindingRows)}

## Node.js Exports

${renderMarkdownTable(['Subpath', 'Runtime entry', 'Types entry'], nodeRows)}

## Go Surface Summary

${renderMarkdownTable(['Metric', 'Value'], goSummaryRows)}

## Go Exported Types

${renderMarkdownTable(['Type'], goTypeRows)}

## Go Commerce Accessors

${renderMarkdownTable(['Accessor'], goAccessorRows)}

## Go API Methods

${renderMarkdownTable(['Receiver', 'Method'], goMethodRows)}

## .NET Surface Summary

${renderMarkdownTable(['Metric', 'Value'], dotnetSummaryRows)}

## .NET Public Types

${renderMarkdownTable(['Type'], dotnetTypeRows)}

## .NET Facade Properties

${renderMarkdownTable(['Property', 'Type'], dotnetFacadeRows)}

## .NET API Methods

${renderMarkdownTable(['API type', 'Method'], dotnetMethodRows)}

## Python Helper Modules

${renderMarkdownTable(['Module'], pythonModuleRows)}

## Python Public Symbols

${renderMarkdownTable(['Symbol'], pythonSymbolRows)}

## Swift Surface Summary

${renderMarkdownTable(['Metric', 'Value'], swiftSummaryRows)}

## Swift Public Types

${renderMarkdownTable(['Type'], swiftTypeRows)}

## Swift Facade Properties

${renderMarkdownTable(['Property', 'Type'], swiftFacadeRows)}

## Swift API Methods

${renderMarkdownTable(['API type', 'Method'], swiftMethodRows)}
`;
}

async function verifyOutput(filePath, expectedContent) {
  const relativePath = path.relative(rootDir, filePath);

  try {
    const actualContent = await readFile(filePath, 'utf8');
    if (!compareStrings(actualContent, expectedContent)) {
      console.error(
        `::error file=${relativePath}::Generated binding API inventory is out of date. Run 'node ./scripts/ci/generate_binding_api_inventory.mjs'.`,
      );
      return false;
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : 'unknown error';
    console.error(
      `::error file=${relativePath}::Unable to read generated binding API inventory output (${message}). Run 'node ./scripts/ci/generate_binding_api_inventory.mjs'.`,
    );
    return false;
  }

  return true;
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
      `Binding API inventory is up to date (${inventory.totalBindings} bindings, ${inventory.detailedBindingCount} detailed surfaces).`,
    );
    return;
  }

  await mkdir(path.dirname(jsonOutputPath), { recursive: true });
  await mkdir(path.dirname(markdownOutputPath), { recursive: true });
  await writeFile(jsonOutputPath, jsonContent, 'utf8');
  await writeFile(markdownOutputPath, markdownContent, 'utf8');

  console.log(
    `Generated binding API inventory (${inventory.totalBindings} bindings, ${inventory.detailedBindingCount} detailed surfaces).`,
  );
}

await main();
