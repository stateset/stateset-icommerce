import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { execSync } from 'node:child_process';
import { promptProjectName, promptStoreName, promptInstall } from './prompts.js';
import { copyTemplate } from './copy.js';
import { print, success, info, error, bold, cyan, dim } from './output.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const TEMPLATE_DIR = path.resolve(__dirname, '..', 'templates', 'storefront');
const PKG = JSON.parse(
  fs.readFileSync(path.resolve(__dirname, '..', 'package.json'), 'utf8'),
);

const HELP = `
  ${bold('create-stateset-app')} — Create a StateSet-powered commerce storefront

  ${dim('Usage:')}
    npx create-stateset-app [project-name] [options]
    npm create stateset-app [project-name] [options]

  ${dim('Options:')}
    --skip-install    Skip npm install
    --use-pnpm        Use pnpm instead of npm
    --use-yarn        Use yarn instead of npm
    -h, --help        Show this help
    -v, --version     Show version

  ${dim('Examples:')}
    npx create-stateset-app my-store
    npx create-stateset-app urban-thread --use-pnpm
    npx create-stateset-app --skip-install
`;

function toTitleCase(slug) {
  return slug
    .split(/[-_]/)
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(' ');
}

function toPackageName(name) {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9-]/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '');
}

function generateEnvExample(targetDir, storeName) {
  const content = `# ${storeName} — Environment Configuration
#
# Copy this file to .env.local and fill in your values:
#   cp .env.example .env.local

# Database path (SQLite via @stateset/embedded)
DATABASE_PATH=./store.db

# Store wallet address for receiving USDC payments (Base chain)
# Get one at https://www.coinbase.com/wallet
NEXT_PUBLIC_STORE_WALLET_ADDRESS=0x0000000000000000000000000000000000000000

# Anthropic API key (required for AI chat assistant)
# Get one at https://console.anthropic.com
ANTHROPIC_API_KEY=

# Base URL for server-side API calls
NEXT_PUBLIC_BASE_URL=http://localhost:3000
`;
  fs.writeFileSync(path.join(targetDir, '.env.example'), content, 'utf8');
}

function printNextSteps(projectName, pm) {
  const run = pm === 'yarn' ? 'yarn' : `${pm} run`;
  print('');
  success(`  Created ${bold(projectName)} successfully!`);
  print('');
  print('  Get started:');
  print('');
  print(`    ${cyan(`cd ${projectName}`)}`);
  print(`    ${cyan('cp .env.example .env.local')}`);
  print(`    ${cyan(`${run} seed`)}`);
  print(`    ${cyan(`${run} dev`)}`);
  print('');
  print(`  Then open ${cyan('http://localhost:3000')}`);
  print('');
  print(`  ${dim('Configure your store by editing .env.local')}`);
  print(`  ${dim('Learn more: https://docs.stateset.io')}`);
  print('');
}

export async function scaffold({ projectName, storeName, targetDir, skipInstall, pm }) {
  const packageName = toPackageName(projectName);

  // Validate target doesn't exist or is empty
  if (fs.existsSync(targetDir)) {
    const entries = fs.readdirSync(targetDir);
    if (entries.length > 0) {
      throw new Error(`Directory "${projectName}" already exists and is not empty.`);
    }
  }

  // Copy template with placeholder replacement
  fs.mkdirSync(targetDir, { recursive: true });
  copyTemplate(TEMPLATE_DIR, targetDir, {
    '{{STORE_NAME}}': storeName,
    '{{PACKAGE_NAME}}': packageName,
  });

  // Generate .env.example
  generateEnvExample(targetDir, storeName);

  // Customize package.json name
  const pkgPath = path.join(targetDir, 'package.json');
  const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
  pkg.name = packageName;
  fs.writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n', 'utf8');

  // Install dependencies
  if (!skipInstall) {
    info(`  Installing dependencies with ${pm}...`);
    print('');
    try {
      execSync(`${pm} install`, { cwd: targetDir, stdio: 'inherit' });
    } catch {
      print('');
      error(`  ${pm} install failed. You can run it manually later.`);
    }
  }
}

export async function main(argv) {
  const { values, positionals } = parseArgs({
    args: argv,
    options: {
      'skip-install': { type: 'boolean', default: false },
      'use-pnpm': { type: 'boolean', default: false },
      'use-yarn': { type: 'boolean', default: false },
      help: { type: 'boolean', short: 'h', default: false },
      version: { type: 'boolean', short: 'v', default: false },
    },
    allowPositionals: true,
  });

  if (values.help) {
    print(HELP);
    return;
  }
  if (values.version) {
    print(PKG.version);
    return;
  }

  print('');
  print(`  ${bold('create-stateset-app')} v${PKG.version}`);
  print('');

  // Get project name
  const rawName = positionals[0] || await promptProjectName();
  const targetDir = path.resolve(process.cwd(), rawName);
  const projectName = path.basename(targetDir);

  // Get store name
  const defaultStoreName = toTitleCase(projectName);
  const storeName = await promptStoreName(defaultStoreName);

  const pm = values['use-pnpm'] ? 'pnpm' : values['use-yarn'] ? 'yarn' : 'npm';
  const skipInstall = values['skip-install'];

  // Check if we should prompt for install
  let shouldInstall = !skipInstall;
  if (shouldInstall && !skipInstall) {
    shouldInstall = await promptInstall(pm);
  }

  print('');
  info(`  Creating ${bold(storeName)} in ${dim(targetDir)}`);

  await scaffold({
    projectName,
    storeName,
    targetDir,
    skipInstall: !shouldInstall,
    pm,
  });

  printNextSteps(projectName, pm);
}
