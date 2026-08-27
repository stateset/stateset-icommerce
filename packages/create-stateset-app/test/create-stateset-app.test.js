import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { scaffold } from '../src/index.js';

const tmpBase = path.join(os.tmpdir(), 'create-stateset-app-tests');

function uniqueDir() {
  const id = `test-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  return path.join(tmpBase, id);
}

before(() => {
  fs.mkdirSync(tmpBase, { recursive: true });
});

after(() => {
  fs.rmSync(tmpBase, { recursive: true, force: true });
});

describe('scaffold', () => {
  it('creates project directory with all expected files', async () => {
    const dir = uniqueDir();
    await scaffold({
      projectName: 'test-store',
      storeName: 'Test Store',
      targetDir: dir,
      skipInstall: true,
      pm: 'npm',
    });

    // Should exist
    assert.ok(fs.existsSync(dir));
    assert.ok(fs.existsSync(path.join(dir, 'package.json')));
    assert.ok(fs.existsSync(path.join(dir, 'next.config.js')));
    assert.ok(fs.existsSync(path.join(dir, 'tsconfig.json')));
    assert.ok(fs.existsSync(path.join(dir, 'tailwind.config.ts')));
    assert.ok(fs.existsSync(path.join(dir, '.env.example')));
    assert.ok(fs.existsSync(path.join(dir, '.gitignore')));
    assert.ok(fs.existsSync(path.join(dir, 'Dockerfile')));
    assert.ok(fs.existsSync(path.join(dir, 'scripts', 'seed.js')));
    assert.ok(fs.existsSync(path.join(dir, 'public', 'placeholder.svg')));

    // App pages
    assert.ok(fs.existsSync(path.join(dir, 'app', 'page.tsx')));
    assert.ok(fs.existsSync(path.join(dir, 'app', 'layout.tsx')));
    assert.ok(fs.existsSync(path.join(dir, 'app', 'products', 'page.tsx')));
    assert.ok(fs.existsSync(path.join(dir, 'app', 'cart', 'page.tsx')));
    assert.ok(fs.existsSync(path.join(dir, 'app', 'checkout', 'page.tsx')));

    // API routes
    assert.ok(fs.existsSync(path.join(dir, 'app', 'api', 'cart', 'route.ts')));
    assert.ok(fs.existsSync(path.join(dir, 'app', 'api', 'checkout', 'route.ts')));
    assert.ok(fs.existsSync(path.join(dir, 'app', 'api', 'chat', 'route.ts')));
    assert.ok(fs.existsSync(path.join(dir, 'app', 'api', 'tax', 'route.ts')));

    // Components
    assert.ok(fs.existsSync(path.join(dir, 'components', 'Providers.tsx')));
    assert.ok(fs.existsSync(path.join(dir, 'components', 'layout', 'Header.tsx')));
    assert.ok(fs.existsSync(path.join(dir, 'components', 'commerce', 'ProductCard.tsx')));

    // Hooks & lib
    assert.ok(fs.existsSync(path.join(dir, 'hooks', 'useCart.tsx')));
    assert.ok(fs.existsSync(path.join(dir, 'lib', 'commerce.ts')));
    assert.ok(fs.existsSync(path.join(dir, 'lib', 'wagmi.ts')));
  });

  it('sets correct package.json name', async () => {
    const dir = uniqueDir();
    await scaffold({
      projectName: 'urban-thread',
      storeName: 'Urban Thread',
      targetDir: dir,
      skipInstall: true,
      pm: 'npm',
    });

    const pkg = JSON.parse(fs.readFileSync(path.join(dir, 'package.json'), 'utf8'));
    assert.equal(pkg.name, 'urban-thread');
  });

  it('uses installable, supported storefront dependencies', async () => {
    const dir = uniqueDir();
    await scaffold({
      projectName: 'dependency-check',
      storeName: 'Dependency Check',
      targetDir: dir,
      skipInstall: true,
      pm: 'npm',
    });

    const pkg = JSON.parse(fs.readFileSync(path.join(dir, 'package.json'), 'utf8'));
    const embeddedPkg = JSON.parse(
      fs.readFileSync(
        path.resolve(import.meta.dirname, '../../../bindings/node/package.json'),
        'utf8',
      ),
    );

    assert.equal(pkg.dependencies['@stateset/embedded'], `^${embeddedPkg.version}`);
    assert.equal(pkg.dependencies.next, '16.3.3');
    assert.equal(pkg.dependencies['better-sqlite3'], undefined);
    assert.equal(pkg.devDependencies['better-sqlite3'], undefined);
    assert.equal(pkg.scripts.lint, undefined);
    assert.equal(pkg.scripts.typecheck, 'tsc --noEmit');
  });

  it('replaces {{STORE_NAME}} placeholders in all files', async () => {
    const dir = uniqueDir();
    await scaffold({
      projectName: 'neon-shop',
      storeName: 'Neon Shop',
      targetDir: dir,
      skipInstall: true,
      pm: 'npm',
    });

    // Check Header
    const header = fs.readFileSync(path.join(dir, 'components', 'layout', 'Header.tsx'), 'utf8');
    assert.ok(header.includes('Neon Shop'), 'Header should contain store name');
    assert.ok(!header.includes('{{STORE_NAME}}'), 'Header should not contain placeholder');

    // Check layout
    const layout = fs.readFileSync(path.join(dir, 'app', 'layout.tsx'), 'utf8');
    assert.ok(layout.includes('Neon Shop'), 'Layout should contain store name');
    assert.ok(!layout.includes('{{STORE_NAME}}'), 'Layout should not contain placeholder');

    // Check Footer
    const footer = fs.readFileSync(path.join(dir, 'components', 'layout', 'Footer.tsx'), 'utf8');
    assert.ok(footer.includes('Neon Shop'), 'Footer should contain store name');
    assert.ok(!footer.includes('{{STORE_NAME}}'), 'Footer should not contain placeholder');

    // Check chat route
    const chat = fs.readFileSync(path.join(dir, 'app', 'api', 'chat', 'route.ts'), 'utf8');
    assert.ok(chat.includes('Neon Shop'), 'Chat route should contain store name');
    assert.ok(!chat.includes('{{STORE_NAME}}'), 'Chat route should not contain placeholder');
  });

  it('replaces {{PACKAGE_NAME}} placeholder', async () => {
    const dir = uniqueDir();
    await scaffold({
      projectName: 'My Cool Store',
      storeName: 'My Cool Store',
      targetDir: dir,
      skipInstall: true,
      pm: 'npm',
    });

    const pkg = JSON.parse(fs.readFileSync(path.join(dir, 'package.json'), 'utf8'));
    assert.equal(pkg.name, 'my-cool-store');
  });

  it('ensures no placeholders remain in any file', async () => {
    const dir = uniqueDir();
    await scaffold({
      projectName: 'sweep-check',
      storeName: 'Sweep Check Store',
      targetDir: dir,
      skipInstall: true,
      pm: 'npm',
    });

    function checkDir(dirPath) {
      const entries = fs.readdirSync(dirPath, { withFileTypes: true });
      for (const entry of entries) {
        const fullPath = path.join(dirPath, entry.name);
        if (entry.isDirectory()) {
          checkDir(fullPath);
        } else {
          const ext = path.extname(entry.name).toLowerCase();
          if (['.ts', '.tsx', '.js', '.jsx', '.json', '.css', '.md'].includes(ext)) {
            const content = fs.readFileSync(fullPath, 'utf8');
            assert.ok(
              !content.includes('{{STORE_NAME}}'),
              `${path.relative(dir, fullPath)} still contains {{STORE_NAME}}`,
            );
            assert.ok(
              !content.includes('{{PACKAGE_NAME}}'),
              `${path.relative(dir, fullPath)} still contains {{PACKAGE_NAME}}`,
            );
          }
        }
      }
    }

    checkDir(dir);
  });

  it('generates .env.example with store name', async () => {
    const dir = uniqueDir();
    await scaffold({
      projectName: 'env-test',
      storeName: 'Env Test Store',
      targetDir: dir,
      skipInstall: true,
      pm: 'npm',
    });

    const envContent = fs.readFileSync(path.join(dir, '.env.example'), 'utf8');
    assert.ok(envContent.includes('Env Test Store'));
    assert.ok(envContent.includes('ANTHROPIC_API_KEY'));
    assert.ok(envContent.includes('NEXT_PUBLIC_STORE_WALLET_ADDRESS'));
  });

  it('throws if target directory is not empty', async () => {
    const dir = uniqueDir();
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'existing-file.txt'), 'hello');

    await assert.rejects(
      () =>
        scaffold({
          projectName: 'blocked',
          storeName: 'Blocked Store',
          targetDir: dir,
          skipInstall: true,
          pm: 'npm',
        }),
      /already exists and is not empty/,
    );
  });

  it('allows creation in an existing empty directory', async () => {
    const dir = uniqueDir();
    fs.mkdirSync(dir, { recursive: true });

    await scaffold({
      projectName: 'empty-ok',
      storeName: 'Empty OK',
      targetDir: dir,
      skipInstall: true,
      pm: 'npm',
    });

    assert.ok(fs.existsSync(path.join(dir, 'package.json')));
  });

  it('ensures no alli-os or pickle references remain', async () => {
    const dir = uniqueDir();
    await scaffold({
      projectName: 'clean-check',
      storeName: 'Clean Check Store',
      targetDir: dir,
      skipInstall: true,
      pm: 'npm',
    });

    function checkDir(dirPath) {
      const entries = fs.readdirSync(dirPath, { withFileTypes: true });
      for (const entry of entries) {
        const fullPath = path.join(dirPath, entry.name);
        if (entry.isDirectory()) {
          checkDir(fullPath);
        } else {
          const ext = path.extname(entry.name).toLowerCase();
          if (['.ts', '.tsx', '.js', '.jsx', '.json', '.css', '.md'].includes(ext)) {
            const content = fs.readFileSync(fullPath, 'utf8').toLowerCase();
            assert.ok(
              !content.includes('alli-os') && !content.includes('alli_os'),
              `${path.relative(dir, fullPath)} contains alli-os reference`,
            );
            assert.ok(
              !content.includes('pickle'),
              `${path.relative(dir, fullPath)} contains pickle reference`,
            );
            assert.ok(
              !content.includes('onions28'),
              `${path.relative(dir, fullPath)} contains onions28 reference`,
            );
          }
        }
      }
    }

    checkDir(dir);
  });

  it('copies binary files (SVG) without corruption', async () => {
    const dir = uniqueDir();
    await scaffold({
      projectName: 'binary-check',
      storeName: 'Binary Check',
      targetDir: dir,
      skipInstall: true,
      pm: 'npm',
    });

    const svg = fs.readFileSync(path.join(dir, 'public', 'placeholder.svg'), 'utf8');
    assert.ok(svg.includes('<svg'));
    assert.ok(svg.includes('</svg>'));
  });

  it('completes scaffold in under 2 seconds', async () => {
    const dir = uniqueDir();
    const start = performance.now();

    await scaffold({
      projectName: 'speed-test',
      storeName: 'Speed Test',
      targetDir: dir,
      skipInstall: true,
      pm: 'npm',
    });

    const elapsed = performance.now() - start;
    assert.ok(elapsed < 2000, `Scaffold took ${elapsed.toFixed(0)}ms (should be <2000ms)`);
  });

  it('produces correct file count', async () => {
    const dir = uniqueDir();
    await scaffold({
      projectName: 'count-check',
      storeName: 'Count Check',
      targetDir: dir,
      skipInstall: true,
      pm: 'npm',
    });

    let count = 0;
    function countFiles(dirPath) {
      const entries = fs.readdirSync(dirPath, { withFileTypes: true });
      for (const entry of entries) {
        if (entry.isDirectory()) {
          countFiles(path.join(dirPath, entry.name));
        } else {
          count++;
        }
      }
    }
    countFiles(dir);

    // 54 template files + .env.example = 55
    assert.ok(count >= 54, `Expected at least 54 files, got ${count}`);
  });

  it('uses stateset_ localStorage keys instead of alli_os_', async () => {
    const dir = uniqueDir();
    await scaffold({
      projectName: 'keys-check',
      storeName: 'Keys Check',
      targetDir: dir,
      skipInstall: true,
      pm: 'npm',
    });

    const useCart = fs.readFileSync(path.join(dir, 'hooks', 'useCart.tsx'), 'utf8');
    assert.ok(useCart.includes('stateset_cart_id'), 'useCart should use stateset_cart_id key');
    assert.ok(!useCart.includes('alli_os_'), 'useCart should not use alli_os_ key');

    const useWishlist = fs.readFileSync(path.join(dir, 'hooks', 'useWishlist.tsx'), 'utf8');
    assert.ok(
      useWishlist.includes('stateset_wishlist'),
      'useWishlist should use stateset_wishlist key',
    );
    assert.ok(!useWishlist.includes('alli_os_'), 'useWishlist should not use alli_os_ key');
  });
});
