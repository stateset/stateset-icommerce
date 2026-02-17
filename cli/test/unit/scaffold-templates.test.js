import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  TEMPLATES,
  PAGE_TEMPLATES,
  COMPONENT_TEMPLATES,
  createPackageJson,
  createTsConfig,
  createNextConfig,
  createTailwindConfig,
  createPostCssConfig,
  createCommerceLib,
  createRootLayout,
  createHomePage,
  createGlobalStyles,
  createGitignore,
  createEnvLocal,
  createReadme,
  generatePageContent,
  generateComponentContent,
  generateHookContent,
  generateApiRouteContent,
  generateSeedScript,
} from '../../src/scaffold-templates.js';

// ============================================================================
// TEMPLATES constant
// ============================================================================

describe('TEMPLATES', () => {
  it('has exactly 4 template entries', () => {
    const keys = Object.keys(TEMPLATES);
    assert.equal(keys.length, 4);
    assert.deepStrictEqual(keys.sort(), ['astro', 'nextjs', 'nextjs-minimal', 'vite-react']);
  });

  it('each template has name, description, framework, and features', () => {
    for (const [key, tmpl] of Object.entries(TEMPLATES)) {
      assert.ok(typeof tmpl.name === 'string' && tmpl.name.length > 0, `${key} missing name`);
      assert.ok(
        typeof tmpl.description === 'string' && tmpl.description.length > 0,
        `${key} missing description`,
      );
      assert.ok(
        typeof tmpl.framework === 'string' && tmpl.framework.length > 0,
        `${key} missing framework`,
      );
      assert.ok(Array.isArray(tmpl.features) && tmpl.features.length > 0, `${key} missing features`);
    }
  });

  it('nextjs template includes ssr and tailwind features', () => {
    assert.ok(TEMPLATES.nextjs.features.includes('ssr'));
    assert.ok(TEMPLATES.nextjs.features.includes('tailwind'));
  });

  it('vite-react template includes wasm feature', () => {
    assert.ok(TEMPLATES['vite-react'].features.includes('wasm'));
  });
});

// ============================================================================
// PAGE_TEMPLATES constant
// ============================================================================

describe('PAGE_TEMPLATES', () => {
  it('has exactly 6 page entries', () => {
    const keys = Object.keys(PAGE_TEMPLATES);
    assert.equal(keys.length, 6);
    assert.deepStrictEqual(
      keys.sort(),
      ['account', 'cart', 'checkout', 'orders', 'product-detail', 'product-listing'],
    );
  });

  it('each page template has name, description, and path', () => {
    for (const [key, page] of Object.entries(PAGE_TEMPLATES)) {
      assert.ok(typeof page.name === 'string' && page.name.length > 0, `${key} missing name`);
      assert.ok(
        typeof page.description === 'string' && page.description.length > 0,
        `${key} missing description`,
      );
      assert.ok(typeof page.path === 'string' && page.path.length > 0, `${key} missing path`);
    }
  });

  it('all paths end with page.tsx', () => {
    for (const [key, page] of Object.entries(PAGE_TEMPLATES)) {
      assert.ok(page.path.endsWith('page.tsx'), `${key} path does not end with page.tsx: ${page.path}`);
    }
  });
});

// ============================================================================
// COMPONENT_TEMPLATES constant
// ============================================================================

describe('COMPONENT_TEMPLATES', () => {
  it('has exactly 7 component entries', () => {
    const keys = Object.keys(COMPONENT_TEMPLATES);
    assert.equal(keys.length, 7);
    assert.deepStrictEqual(
      keys.sort(),
      ['add-to-cart', 'cart-drawer', 'checkout-form', 'footer', 'header', 'product-card', 'product-grid'],
    );
  });

  it('each component template has name, description, and path', () => {
    for (const [key, comp] of Object.entries(COMPONENT_TEMPLATES)) {
      assert.ok(typeof comp.name === 'string' && comp.name.length > 0, `${key} missing name`);
      assert.ok(
        typeof comp.description === 'string' && comp.description.length > 0,
        `${key} missing description`,
      );
      assert.ok(typeof comp.path === 'string' && comp.path.length > 0, `${key} missing path`);
    }
  });

  it('component paths are under components/ directory', () => {
    for (const [key, comp] of Object.entries(COMPONENT_TEMPLATES)) {
      assert.ok(
        comp.path.startsWith('components/'),
        `${key} path not under components/: ${comp.path}`,
      );
    }
  });
});

// ============================================================================
// createPackageJson
// ============================================================================

describe('createPackageJson', () => {
  it('returns an object with expected top-level fields', () => {
    const pkg = createPackageJson('My Store', 'nextjs', []);
    assert.equal(typeof pkg, 'object');
    assert.ok('name' in pkg);
    assert.ok('version' in pkg);
    assert.ok('private' in pkg);
    assert.ok('scripts' in pkg);
    assert.ok('dependencies' in pkg);
    assert.ok('devDependencies' in pkg);
  });

  it('sanitizes name to lowercase with hyphens only', () => {
    const pkg = createPackageJson('My Awesome Store!!! 123', 'nextjs', []);
    assert.equal(pkg.name, 'my-awesome-store----123');
    assert.ok(/^[a-z0-9-]+$/.test(pkg.name));
  });

  it('lowercases uppercase names', () => {
    const pkg = createPackageJson('UPPERCASE', 'nextjs', []);
    assert.equal(pkg.name, 'uppercase');
  });

  it('replaces special characters with hyphens', () => {
    const pkg = createPackageJson('store@v2.0', 'nextjs', []);
    assert.equal(pkg.name, 'store-v2-0');
  });

  it('includes tailwind devDependencies when template has tailwind feature', () => {
    const pkg = createPackageJson('store', 'nextjs', []);
    assert.ok('tailwindcss' in pkg.devDependencies);
    assert.ok('autoprefixer' in pkg.devDependencies);
    assert.ok('postcss' in pkg.devDependencies);
  });

  it('includes tailwind devDependencies when features array contains tailwind', () => {
    const pkg = createPackageJson('store', 'nextjs-minimal', ['tailwind']);
    assert.ok('tailwindcss' in pkg.devDependencies);
    assert.ok('autoprefixer' in pkg.devDependencies);
    assert.ok('postcss' in pkg.devDependencies);
  });

  it('omits tailwind devDependencies when neither template nor features include tailwind', () => {
    const pkg = createPackageJson('store', 'nextjs-minimal', []);
    assert.ok(!('tailwindcss' in pkg.devDependencies));
    assert.ok(!('autoprefixer' in pkg.devDependencies));
  });

  it('always includes @stateset/embedded dependency', () => {
    const pkg = createPackageJson('store', 'nextjs', []);
    assert.ok('@stateset/embedded' in pkg.dependencies);
  });

  it('includes seed script', () => {
    const pkg = createPackageJson('store', 'nextjs', []);
    assert.equal(pkg.scripts.seed, 'node scripts/seed.js');
  });

  it('marks package as private', () => {
    const pkg = createPackageJson('store', 'nextjs', []);
    assert.equal(pkg.private, true);
  });
});

// ============================================================================
// createTsConfig
// ============================================================================

describe('createTsConfig', () => {
  it('returns valid JSON', () => {
    const result = createTsConfig('nextjs');
    const parsed = JSON.parse(result);
    assert.ok(typeof parsed === 'object');
  });

  it('contains compilerOptions', () => {
    const parsed = JSON.parse(createTsConfig('nextjs'));
    assert.ok('compilerOptions' in parsed);
    assert.equal(parsed.compilerOptions.strict, true);
  });

  it('contains include and exclude arrays', () => {
    const parsed = JSON.parse(createTsConfig('nextjs'));
    assert.ok(Array.isArray(parsed.include));
    assert.ok(Array.isArray(parsed.exclude));
    assert.ok(parsed.exclude.includes('node_modules'));
  });

  it('uses bundler module resolution', () => {
    const parsed = JSON.parse(createTsConfig('nextjs'));
    assert.equal(parsed.compilerOptions.moduleResolution, 'bundler');
  });
});

// ============================================================================
// Config generators (string outputs)
// ============================================================================

describe('createNextConfig', () => {
  it('returns a non-empty string', () => {
    const result = createNextConfig();
    assert.ok(typeof result === 'string');
    assert.ok(result.length > 0);
  });

  it('includes remote image pattern for unsplash', () => {
    assert.ok(createNextConfig().includes('images.unsplash.com'));
  });

  it('references @stateset/embedded in experimental config', () => {
    assert.ok(createNextConfig().includes('@stateset/embedded'));
  });
});

describe('createTailwindConfig', () => {
  it('returns a non-empty string', () => {
    const result = createTailwindConfig();
    assert.ok(typeof result === 'string');
    assert.ok(result.length > 0);
  });

  it('includes content globs for app, pages, and components', () => {
    const config = createTailwindConfig();
    assert.ok(config.includes('./app/**'));
    assert.ok(config.includes('./pages/**'));
    assert.ok(config.includes('./components/**'));
  });
});

describe('createPostCssConfig', () => {
  it('returns a non-empty string referencing tailwindcss and autoprefixer', () => {
    const result = createPostCssConfig();
    assert.ok(result.length > 0);
    assert.ok(result.includes('tailwindcss'));
    assert.ok(result.includes('autoprefixer'));
  });
});

describe('createCommerceLib', () => {
  it('returns a non-empty string', () => {
    const result = createCommerceLib();
    assert.ok(result.length > 0);
  });

  it('imports from @stateset/embedded', () => {
    assert.ok(createCommerceLib().includes("from '@stateset/embedded'"));
  });

  it('exports getProducts and getProduct functions', () => {
    const lib = createCommerceLib();
    assert.ok(lib.includes('export async function getProducts'));
    assert.ok(lib.includes('export async function getProduct'));
  });
});

describe('createHomePage', () => {
  it('returns a non-empty string', () => {
    assert.ok(createHomePage().length > 0);
  });

  it('imports getProducts from commerce lib', () => {
    assert.ok(createHomePage().includes("from '@/lib/commerce'"));
  });
});

describe('createGlobalStyles', () => {
  it('returns a non-empty string with tailwind directives', () => {
    const css = createGlobalStyles();
    assert.ok(css.includes('@tailwind base'));
    assert.ok(css.includes('@tailwind components'));
    assert.ok(css.includes('@tailwind utilities'));
  });
});

describe('createGitignore', () => {
  it('includes common ignores', () => {
    const gi = createGitignore();
    assert.ok(gi.includes('node_modules/'));
    assert.ok(gi.includes('.next/'));
    assert.ok(gi.includes('.env'));
    assert.ok(gi.includes('.DS_Store'));
  });

  it('ignores database files', () => {
    const gi = createGitignore();
    assert.ok(gi.includes('*.db'));
    assert.ok(gi.includes('*.sqlite'));
  });
});

describe('createEnvLocal', () => {
  it('returns a non-empty string with DATABASE_PATH', () => {
    const env = createEnvLocal();
    assert.ok(env.includes('DATABASE_PATH'));
  });
});

// ============================================================================
// createRootLayout
// ============================================================================

describe('createRootLayout', () => {
  it('includes the provided name in the title metadata', () => {
    const layout = createRootLayout('Urban Thread');
    assert.ok(layout.includes("title: 'Urban Thread'"));
  });

  it('includes the provided name in the nav link', () => {
    const layout = createRootLayout('Cool Shop');
    assert.ok(layout.includes('>Cool Shop</a>'));
  });

  it('contains iCommerce branding', () => {
    assert.ok(createRootLayout('test').includes('Powered by StateSet iCommerce'));
  });
});

// ============================================================================
// createReadme
// ============================================================================

describe('createReadme', () => {
  it('includes the project name as heading', () => {
    const md = createReadme('Urban Thread', 'nextjs');
    assert.ok(md.startsWith('# Urban Thread'));
  });

  it('includes the template framework name from TEMPLATES', () => {
    const md = createReadme('My Shop', 'nextjs');
    assert.ok(md.includes('Next.js 14 Storefront'));
  });

  it('falls back to Next.js when template is unknown', () => {
    const md = createReadme('My Shop', 'unknown-template');
    assert.ok(md.includes('Next.js'));
  });

  it('includes the project name in the directory structure', () => {
    const md = createReadme('Acme Store', 'nextjs');
    assert.ok(md.includes('Acme Store/'));
  });

  it('includes getting started instructions', () => {
    const md = createReadme('Shop', 'nextjs');
    assert.ok(md.includes('npm install'));
    assert.ok(md.includes('npm run dev'));
    assert.ok(md.includes('npm run seed'));
  });
});

// ============================================================================
// generatePageContent
// ============================================================================

describe('generatePageContent', () => {
  it('returns product-listing template', () => {
    const content = generatePageContent('product-listing');
    assert.ok(content.includes('ProductsPage'));
    assert.ok(content.includes('getProducts'));
  });

  it('returns product-detail template', () => {
    const content = generatePageContent('product-detail');
    assert.ok(content.includes('ProductPage'));
    assert.ok(content.includes('getProductBySlug'));
    assert.ok(content.includes('notFound'));
  });

  it('returns cart template with use client directive', () => {
    const content = generatePageContent('cart');
    assert.ok(content.includes("'use client'"));
    assert.ok(content.includes('CartPage'));
  });

  it('returns checkout template', () => {
    const content = generatePageContent('checkout');
    assert.ok(content.includes('CheckoutPage'));
  });

  it('returns account template', () => {
    const content = generatePageContent('account');
    assert.ok(content.includes('AccountPage'));
    assert.ok(content.includes('My Account'));
  });

  it('returns orders template', () => {
    const content = generatePageContent('orders');
    assert.ok(content.includes('OrdersPage'));
    assert.ok(content.includes('Order History'));
  });

  it('returns fallback with customName for unknown page types', () => {
    const content = generatePageContent('unknown-page', 'MyCustomPage');
    assert.ok(content.includes('MyCustomPage'));
  });

  it('returns fallback with default Page name when no customName', () => {
    const content = generatePageContent('unknown-page');
    assert.ok(content.includes('function Page()'));
  });
});

// ============================================================================
// generateComponentContent
// ============================================================================

describe('generateComponentContent', () => {
  it('returns product-card template', () => {
    const content = generateComponentContent('product-card');
    assert.ok(content.includes('ProductCard'));
    assert.ok(content.includes('ProductCardProps'));
  });

  it('returns add-to-cart template with use client', () => {
    const content = generateComponentContent('add-to-cart');
    assert.ok(content.includes("'use client'"));
    assert.ok(content.includes('AddToCartButton'));
  });

  it('returns header template', () => {
    const content = generateComponentContent('header');
    assert.ok(content.includes('function Header'));
  });

  it('returns footer template', () => {
    const content = generateComponentContent('footer');
    assert.ok(content.includes('function Footer'));
    assert.ok(content.includes('StateSet iCommerce'));
  });

  it('returns fallback with customName for unknown component types', () => {
    const content = generateComponentContent('unknown-comp', 'Widget');
    assert.ok(content.includes('function Widget'));
    assert.ok(content.includes('WidgetProps'));
  });

  it('returns fallback with default Component name when no customName', () => {
    const content = generateComponentContent('unknown-comp');
    assert.ok(content.includes('function Component'));
    assert.ok(content.includes('ComponentProps'));
  });
});

// ============================================================================
// generateHookContent
// ============================================================================

describe('generateHookContent', () => {
  it('returns useCart hook template', () => {
    const content = generateHookContent('useCart');
    assert.ok(content.includes('function useCart'));
    assert.ok(content.includes('localStorage'));
    assert.ok(content.includes('addItem'));
    assert.ok(content.includes('removeItem'));
  });

  it('returns useProducts hook template', () => {
    const content = generateHookContent('useProducts');
    assert.ok(content.includes('function useProducts'));
    assert.ok(content.includes('setProducts'));
    assert.ok(content.includes('/api/products'));
  });

  it('returns fallback with customName for unknown hooks', () => {
    const content = generateHookContent('useWishlist', 'useWishlist');
    assert.ok(content.includes('function useWishlist'));
    assert.ok(content.includes('useState'));
  });

  it('uses hookName in fallback when no customName', () => {
    const content = generateHookContent('useSearch');
    assert.ok(content.includes('function useSearch'));
  });
});

// ============================================================================
// generateApiRouteContent
// ============================================================================

describe('generateApiRouteContent', () => {
  it('includes all specified HTTP methods', () => {
    const content = generateApiRouteContent('/api/products', ['GET', 'POST']);
    assert.ok(content.includes('export async function GET'));
    assert.ok(content.includes('export async function POST'));
  });

  it('references the generated route path in generated content', () => {
    const content = generateApiRouteContent('/api/orders', ['GET']);
    assert.ok(content.includes('/api/orders'));
  });

  it('contains reusable request dispatcher helpers', () => {
    const content = generateApiRouteContent('/api/products', ['GET']);
    assert.ok(content.includes('resolveResourceManager'));
    assert.ok(content.includes('parseEntityId'));
    assert.ok(content.includes('routeRequest'));
  });

  it('includes fallback resource method checks', () => {
    const content = generateApiRouteContent('/api/products', ['GET', 'POST', 'PUT', 'PATCH', 'DELETE']);
    assert.ok(content.includes('resource.get || resource.getById || resource.findById'));
    assert.ok(content.includes('resource.create'));
    assert.ok(content.includes('resource.update || resource.save || resource.patch'));
    assert.ok(content.includes('resource.delete || resource.remove || resource.destroy'));
  });

  it('does not include the old TODO marker in generated route handlers', () => {
    const content = generateApiRouteContent('/api/products', ['GET']);
    assert.ok(!content.includes('TODO:'));
  });

  it('imports NextRequest and NextResponse', () => {
    const content = generateApiRouteContent('/api/carts', ['GET']);
    assert.ok(content.includes("import { NextRequest, NextResponse } from 'next/server'"));
  });

  it('imports getCommerce from commerce lib', () => {
    const content = generateApiRouteContent('/api/carts', ['GET']);
    assert.ok(content.includes("import { getCommerce } from '@/lib/commerce'"));
  });

  it('handles multiple methods (GET, POST, PUT, DELETE)', () => {
    const methods = ['GET', 'POST', 'PUT', 'DELETE'];
    const content = generateApiRouteContent('/api/items', methods);
    for (const m of methods) {
      assert.ok(content.includes(`export async function ${m}`), `Missing handler for ${m}`);
    }
  });

  it('includes error handling in each handler', () => {
    const content = generateApiRouteContent('/api/test', ['GET']);
    assert.ok(content.includes('catch (error)'));
    assert.ok(content.includes('status: 500'));
  });
});

// ============================================================================
// generateSeedScript
// ============================================================================

describe('generateSeedScript', () => {
  it('includes the provided database path', () => {
    const script = generateSeedScript('./my-store.db', 5);
    assert.ok(script.includes('./my-store.db'));
  });

  it('includes the provided product count in the slice call', () => {
    const script = generateSeedScript('./store.db', 3);
    assert.ok(script.includes('.slice(0, 3)'));
  });

  it('uses require for @stateset/embedded (CommonJS)', () => {
    const script = generateSeedScript('./store.db', 5);
    assert.ok(script.includes("require('@stateset/embedded')"));
  });

  it('includes seed products with names and prices', () => {
    const script = generateSeedScript('./store.db', 10);
    assert.ok(script.includes('Classic T-Shirt'));
    assert.ok(script.includes('Premium Hoodie'));
    assert.ok(script.includes('29.99'));
  });

  it('creates inventory items alongside products', () => {
    const script = generateSeedScript('./store.db', 5);
    assert.ok(script.includes('commerce.inventory.createItem'));
    assert.ok(script.includes('initialQuantity: 100'));
  });

  it('handles custom dbPath with special characters', () => {
    const script = generateSeedScript('/data/stores/my-shop.db', 2);
    assert.ok(script.includes('/data/stores/my-shop.db'));
    assert.ok(script.includes('.slice(0, 2)'));
  });
});
