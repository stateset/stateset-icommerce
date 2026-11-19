#!/usr/bin/env node
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { parseArgs } from 'node:util';
import { fileURLToPath } from 'node:url';
import { scaffold } from '../packages/create-stateset-app/src/index.js';

const version = '1.40.1';
const checksum = '29775b573dd1ef655b115af81a40a2f5ae2215827dbbdbd508e6d59139c924df';
const archive = 'stateset-storefront-1.40.1-linux-x64-gnu.tar';
const release = 'https://raw.githubusercontent.com/stateset/stateset-icommerce/0637ef6e8a6048833e609cb5006bc0f1d8ac7e60';
const url = release + '/artifacts/storefront-1.40.1-linux-x64-gnu-29775b573dd1/' + archive;
const packages = ['@stateset/embedded', '@stateset/embedded-linux-x64-gnu'];

async function install() {
  const { positionals, values } = parseArgs({
    options: { bundle: { type: 'string' }, 'lock-only': { type: 'boolean' }, help: { type: 'boolean', short: 'h' } },
    allowPositionals: true,
  });
  if (values.help || positionals.length !== 1) {
    console.log('Usage: npm run storefront:preview -- PROJECT_DIRECTORY [--bundle PATH_TO_VERIFIED_TAR] [--lock-only]');
    if (!values.help) process.exitCode = 1;
    return;
  }
  const targetDir = path.resolve(positionals[0]);
  if (existsSync(targetDir)) throw new Error('Choose a new project directory: ' + targetDir);
  const template = new URL('../packages/create-stateset-app/templates/storefront/package.json', import.meta.url);
  if (JSON.parse(readFileSync(template, 'utf8')).dependencies['@stateset/embedded'] !== '^' + version) {
    throw new Error('This storefront requires a different SDK version from the verified preview.');
  }
  const report = process.report.getReport().header;
  if (process.platform !== 'linux' || process.arch !== 'x64' || !report.glibcVersionRuntime) {
    throw new Error('The verified preview requires Linux x64 with glibc. Use npm create stateset-app@latest for the published storefront.');
  }

  const temporary = mkdtempSync(path.join(tmpdir(), 'stateset-preview-'));
  try {
    let bundle;
    if (values.bundle) {
      bundle = path.resolve(values.bundle);
    } else {
      console.log('Downloading the verified StateSet ' + version + ' Linux preview SDK...');
      const response = await fetch(url, { signal: AbortSignal.timeout(120_000) });
      if (!response.ok) throw new Error('Preview download failed with HTTP ' + response.status);
      bundle = path.join(temporary, archive);
      writeFileSync(bundle, Buffer.from(await response.arrayBuffer()));
    }
    const digest = createHash('sha256').update(readFileSync(bundle)).digest('hex');
    if (digest !== checksum) throw new Error('The preview archive failed its SHA-256 check.');

    const projectName = path.basename(targetDir);
    // Reserve a fresh directory before the generator writes any user files.
    mkdirSync(targetDir);
    await scaffold({ projectName, storeName: projectName, targetDir, skipInstall: true, pm: 'npm' });
    const manifestPath = path.join(targetDir, 'package.json');
    const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
    const vendorDir = 'vendor/stateset';
    mkdirSync(path.join(targetDir, vendorDir), { recursive: true });
    const verification = execFileSync('tar', ['-xOf', bundle, 'verification.json']);
    const provenance = { archive, url, sha256: checksum, buildEvidence: JSON.parse(verification) };
    writeFileSync(path.join(targetDir, vendorDir, 'SDK-VERIFICATION.json'), JSON.stringify(provenance, null, 2) + '\n');
    const notices = path.join(targetDir, vendorDir, 'notices');
    const nativeNotices = path.join(notices, 'native');
    mkdirSync(nativeNotices, { recursive: true });
    const noticeArchive = new URL('./storefront-native-notices/linux-x64-gnu-notices.tar.gz', import.meta.url);
    execFileSync('tar', ['-xzf', fileURLToPath(noticeArchive), '-C', nativeNotices]);
    const inventory = JSON.parse(readFileSync(path.join(nativeNotices, 'inventory.json'), 'utf8'));
    if (inventory.build.archiveSha256 !== provenance.buildEvidence.packages[packages[1]].sha256) {
      throw new Error('The native notices belong to a different SDK build.');
    }
    for (const license of ['LICENSE', 'LICENSE-MIT', 'LICENSE-APACHE']) {
      copyFileSync(new URL('../' + license, import.meta.url), path.join(notices, license));
    }
    for (const name of packages) {
      const filename = name.replace('@', '').replace('/', '-') + '-' + version + '.tgz';
      // Extract only the named npm archive. npm also records its integrity in the project lockfile.
      const tarball = execFileSync('tar', ['-xOf', bundle, filename], { maxBuffer: 64 * 1024 * 1024 });
      writeFileSync(path.join(targetDir, vendorDir, filename), tarball);
      manifest.dependencies[name] = 'file:' + vendorDir + '/' + filename;
    }
    writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + '\n');

    const dockerPath = path.join(targetDir, 'Dockerfile');
    const dockerfile = readFileSync(dockerPath, 'utf8');
    const copyManifest = 'COPY package.json package-lock.json* ./';
    if (!dockerfile.includes(copyManifest)) throw new Error('Cannot locate the Docker dependency stage.');
    const runtime = 'FROM base AS runtime';
    if (!dockerfile.includes(runtime)) throw new Error('Cannot locate the Docker runtime stage.');
    writeFileSync(dockerPath, dockerfile
      .replace(copyManifest, copyManifest + '\nCOPY vendor/stateset/ ./vendor/stateset/')
      .replace(runtime, runtime + '\nCOPY vendor/stateset/notices/ /usr/share/doc/stateset/'));
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }

  try {
    const args = ['install', '--include=optional'];
    if (values['lock-only']) args.push('--package-lock-only', '--ignore-scripts');
    execFileSync('npm', args, { cwd: targetDir, stdio: 'inherit' });
  } catch {
    throw new Error('The project and verified SDK are in ' + targetDir + '. Run npm install there to retry.');
  }
  console.log('Storefront ready in ' + targetDir + '.');
  if (values['lock-only']) console.log('Run npm ci --include=optional there to install the locked dependencies.');
  else console.log('Run npm run verify:storefront there to exercise the local merchant workflows.');
}

install().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
