/**
 * Scaffolding MCP Server for StateSet Storefront Creation
 *
 * Provides file system tools for creating e-commerce storefronts
 * using StateSet iCommerce engine.
 */

import { createSdkMcpServer, tool } from '@anthropic-ai/claude-agent-sdk';
import { z } from 'zod';
import fs from 'node:fs';
import path from 'node:path';
import { execSync, spawn } from 'node:child_process';

// ============================================================================
// Project Templates
// ============================================================================

const TEMPLATES = {
  'nextjs': {
    name: 'Next.js 14 Storefront',
    description: 'Full-stack React storefront with App Router, SSR, and Tailwind CSS',
    framework: 'next',
    features: ['ssr', 'api-routes', 'tailwind', 'typescript'],
  },
  'nextjs-minimal': {
    name: 'Next.js Minimal',
    description: 'Minimal Next.js storefront with basic pages',
    framework: 'next',
    features: ['typescript'],
  },
  'vite-react': {
    name: 'Vite + React SPA',
    description: 'Client-side React SPA using WASM bindings',
    framework: 'vite',
    features: ['wasm', 'tailwind', 'typescript'],
  },
  'astro': {
    name: 'Astro Storefront',
    description: 'Static-first storefront with Islands architecture',
    framework: 'astro',
    features: ['static', 'islands', 'tailwind'],
  },
};

const PAGE_TEMPLATES = {
  'product-listing': {
    name: 'Product Listing Page',
    description: 'Grid of products with filters and pagination',
    path: 'app/products/page.tsx',
  },
  'product-detail': {
    name: 'Product Detail Page',
    description: 'Single product view with add to cart',
    path: 'app/products/[slug]/page.tsx',
  },
  'cart': {
    name: 'Shopping Cart Page',
    description: 'Cart with items, quantities, and totals',
    path: 'app/cart/page.tsx',
  },
  'checkout': {
    name: 'Checkout Page',
    description: 'Multi-step checkout flow',
    path: 'app/checkout/page.tsx',
  },
  'account': {
    name: 'Account Dashboard',
    description: 'Customer account overview',
    path: 'app/account/page.tsx',
  },
  'orders': {
    name: 'Order History',
    description: 'List of customer orders',
    path: 'app/account/orders/page.tsx',
  },
};

const COMPONENT_TEMPLATES = {
  'product-card': {
    name: 'ProductCard',
    description: 'Product card for grid display',
    path: 'components/commerce/ProductCard.tsx',
  },
  'product-grid': {
    name: 'ProductGrid',
    description: 'Responsive grid of product cards',
    path: 'components/commerce/ProductGrid.tsx',
  },
  'cart-drawer': {
    name: 'CartDrawer',
    description: 'Slide-out cart drawer',
    path: 'components/commerce/CartDrawer.tsx',
  },
  'add-to-cart': {
    name: 'AddToCartButton',
    description: 'Add to cart button with loading state',
    path: 'components/commerce/AddToCartButton.tsx',
  },
  'checkout-form': {
    name: 'CheckoutForm',
    description: 'Multi-step checkout form',
    path: 'components/commerce/CheckoutForm.tsx',
  },
  'header': {
    name: 'Header',
    description: 'Site header with navigation and cart',
    path: 'components/layout/Header.tsx',
  },
  'footer': {
    name: 'Footer',
    description: 'Site footer with links',
    path: 'components/layout/Footer.tsx',
  },
};

// ============================================================================
// Helper Functions
// ============================================================================

function ensureDir(dirPath) {
  if (!fs.existsSync(dirPath)) {
    fs.mkdirSync(dirPath, { recursive: true });
  }
}

function writeFileSync(filePath, content) {
  ensureDir(path.dirname(filePath));
  fs.writeFileSync(filePath, content, 'utf8');
}

function fileExists(filePath) {
  return fs.existsSync(filePath);
}

function readFileSync(filePath) {
  return fs.readFileSync(filePath, 'utf8');
}

// Helper to create MCP CallToolResult
function result(data) {
  return {
    content: [{ type: 'text', text: JSON.stringify(data, null, 2) }]
  };
}

function errorResult(message) {
  return {
    content: [{ type: 'text', text: JSON.stringify({ success: false, error: message }, null, 2) }],
    isError: true
  };
}

// ============================================================================
// Create Scaffolding MCP Server
// ============================================================================

/**
 * Create the scaffolding MCP server
 * @param {Object} options
 * @param {string} options.workDir - Working directory for file operations
 * @param {boolean} options.allowWrite - Whether to allow write operations
 */
export function createScaffoldMcpServer({ workDir = process.cwd(), allowWrite = false }) {
  return createSdkMcpServer({
    name: 'stateset-scaffold',
    version: '1.0.0',
    tools: [
      // ========================================
      // Project Management Tools
      // ========================================

      tool(
        'list_templates',
        'List available storefront project templates',
        {},
        async () => {
          return result({
            success: true,
            templates: Object.entries(TEMPLATES).map(([id, t]) => ({
              id,
              name: t.name,
              description: t.description,
              framework: t.framework,
              features: t.features,
            })),
          });
        }
      ),

      tool(
        'list_page_templates',
        'List available page templates',
        {},
        async () => {
          return result({
            success: true,
            pages: Object.entries(PAGE_TEMPLATES).map(([id, p]) => ({
              id,
              name: p.name,
              description: p.description,
              path: p.path,
            })),
          });
        }
      ),

      tool(
        'list_component_templates',
        'List available component templates',
        {},
        async () => {
          return result({
            success: true,
            components: Object.entries(COMPONENT_TEMPLATES).map(([id, c]) => ({
              id,
              name: c.name,
              description: c.description,
              path: c.path,
            })),
          });
        }
      ),

      tool(
        'create_project',
        'Create a new storefront project with the specified template. This initializes the full project structure.',
        {
          name: z.string().describe('Project name (used for directory and package name)'),
          template: z.enum(['nextjs', 'nextjs-minimal', 'vite-react', 'astro']).describe('Project template to use'),
          directory: z.string().optional().describe('Directory to create project in (defaults to current directory)'),
        },
        async ({ name, template, directory }) => {
          if (!allowWrite) {
            return result({
              success: false,
              preview: true,
              message: `Would create ${template} project "${name}" in ${directory || workDir}/${name}`,
              template: TEMPLATES[template],
            });
          }

          const projectDir = path.join(directory || workDir, name);

          if (fs.existsSync(projectDir)) {
            return errorResult(`Directory ${projectDir} already exists`);
          }

          ensureDir(projectDir);

          // Create package.json
          const packageJson = createPackageJson(name, template, []);
          writeFileSync(path.join(projectDir, 'package.json'), JSON.stringify(packageJson, null, 2));

          // Create TypeScript config
          writeFileSync(path.join(projectDir, 'tsconfig.json'), createTsConfig(template));

          // Create Next.js config (if applicable)
          if (template.startsWith('next')) {
            writeFileSync(path.join(projectDir, 'next.config.js'), createNextConfig());
          }

          // Create Tailwind config
          if (TEMPLATES[template].features.includes('tailwind')) {
            writeFileSync(path.join(projectDir, 'tailwind.config.ts'), createTailwindConfig());
            writeFileSync(path.join(projectDir, 'postcss.config.js'), createPostCssConfig());
          }

          // Create directory structure
          const dirs = [
            'app',
            'app/api',
            'app/products',
            'app/cart',
            'app/checkout',
            'components',
            'components/ui',
            'components/commerce',
            'components/layout',
            'lib',
            'hooks',
            'public',
            'styles',
          ];

          for (const dir of dirs) {
            ensureDir(path.join(projectDir, dir));
          }

          // Create base files
          writeFileSync(path.join(projectDir, 'lib/commerce.ts'), createCommerceLib());
          writeFileSync(path.join(projectDir, 'app/layout.tsx'), createRootLayout(name));
          writeFileSync(path.join(projectDir, 'app/page.tsx'), createHomePage());
          writeFileSync(path.join(projectDir, 'styles/globals.css'), createGlobalStyles());
          writeFileSync(path.join(projectDir, '.gitignore'), createGitignore());
          writeFileSync(path.join(projectDir, '.env.local'), createEnvLocal());
          writeFileSync(path.join(projectDir, 'README.md'), createReadme(name, template));

          return result({
            success: true,
            message: `Created ${template} project "${name}"`,
            projectDir,
            nextSteps: [
              `cd ${name}`,
              'npm install',
              'npm run dev',
            ],
          });
        }
      ),

      tool(
        'add_page',
        'Add a page to the storefront project',
        {
          pageType: z.enum(['product-listing', 'product-detail', 'cart', 'checkout', 'account', 'orders', 'custom']).describe('Type of page to add'),
          customPath: z.string().optional().describe('Custom path for the page (only for custom type)'),
          customName: z.string().optional().describe('Custom name for the page (only for custom type)'),
        },
        async ({ pageType, customPath, customName }) => {
          const template = PAGE_TEMPLATES[pageType];
          const pagePath = pageType === 'custom' ? customPath : template?.path;

          if (!pagePath) {
            return errorResult('Invalid page type or missing custom path');
          }

          if (!allowWrite) {
            return result({
              success: false,
              preview: true,
              message: `Would create page at ${pagePath}`,
              template: template?.name || customName,
            });
          }

          const fullPath = path.join(workDir, pagePath);
          const content = generatePageContent(pageType, customName);

          writeFileSync(fullPath, content);

          return result({
            success: true,
            message: `Created ${template?.name || customName} page`,
            path: pagePath,
          });
        }
      ),

      tool(
        'add_component',
        'Add a component to the storefront project',
        {
          componentType: z.enum(['product-card', 'product-grid', 'cart-drawer', 'add-to-cart', 'checkout-form', 'header', 'footer', 'custom']).describe('Type of component to add'),
          customPath: z.string().optional().describe('Custom path for the component (only for custom type)'),
          customName: z.string().optional().describe('Custom name for the component (only for custom type)'),
        },
        async ({ componentType, customPath, customName }) => {
          const template = COMPONENT_TEMPLATES[componentType];
          const componentPath = componentType === 'custom' ? customPath : template?.path;

          if (!componentPath) {
            return errorResult('Invalid component type or missing custom path');
          }

          if (!allowWrite) {
            return result({
              success: false,
              preview: true,
              message: `Would create component at ${componentPath}`,
              template: template?.name || customName,
            });
          }

          const fullPath = path.join(workDir, componentPath);
          const content = generateComponentContent(componentType, customName);

          writeFileSync(fullPath, content);

          return result({
            success: true,
            message: `Created ${template?.name || customName} component`,
            path: componentPath,
          });
        }
      ),

      tool(
        'add_hook',
        'Add a React hook to the storefront project',
        {
          hookName: z.enum(['useCart', 'useProducts', 'useCheckout', 'useCustomer', 'custom']).describe('Name of the hook to add'),
          customName: z.string().optional().describe('Custom hook name (only for custom type)'),
        },
        async ({ hookName, customName }) => {
          const name = hookName === 'custom' ? customName : hookName;
          if (!name) {
            return errorResult('Hook name is required');
          }

          const hookPath = `hooks/${name}.ts`;

          if (!allowWrite) {
            return result({
              success: false,
              preview: true,
              message: `Would create hook at ${hookPath}`,
            });
          }

          const fullPath = path.join(workDir, hookPath);
          const content = generateHookContent(hookName, customName);

          writeFileSync(fullPath, content);

          return result({
            success: true,
            message: `Created ${name} hook`,
            path: hookPath,
          });
        }
      ),

      tool(
        'add_api_route',
        'Add an API route to the storefront project',
        {
          routePath: z.string().describe('API route path (e.g., "products", "cart", "checkout")'),
          methods: z.array(z.enum(['GET', 'POST', 'PUT', 'PATCH', 'DELETE'])).describe('HTTP methods to support'),
        },
        async ({ routePath, methods }) => {
          const apiPath = `app/api/${routePath}/route.ts`;

          if (!allowWrite) {
            return result({
              success: false,
              preview: true,
              message: `Would create API route at ${apiPath}`,
              methods,
            });
          }

          const fullPath = path.join(workDir, apiPath);
          const content = generateApiRouteContent(routePath, methods);

          writeFileSync(fullPath, content);

          return result({
            success: true,
            message: `Created API route at ${apiPath}`,
            path: apiPath,
            methods,
          });
        }
      ),

      tool(
        'write_file',
        'Write content to a file in the project',
        {
          filePath: z.string().describe('Path to the file relative to project root'),
          content: z.string().describe('Content to write to the file'),
          overwrite: z.boolean().optional().describe('Whether to overwrite existing file'),
        },
        async ({ filePath, content, overwrite }) => {
          const fullPath = path.join(workDir, filePath);

          if (!allowWrite) {
            return result({
              success: false,
              preview: true,
              message: `Would write ${content.length} characters to ${filePath}`,
            });
          }

          if (fileExists(fullPath) && !overwrite) {
            return errorResult(`File ${filePath} already exists. Set overwrite: true to replace.`);
          }

          writeFileSync(fullPath, content);

          return result({
            success: true,
            message: `Wrote ${content.length} characters to ${filePath}`,
            path: filePath,
          });
        }
      ),

      tool(
        'read_file',
        'Read content from a file in the project',
        {
          filePath: z.string().describe('Path to the file relative to project root'),
        },
        async ({ filePath }) => {
          const fullPath = path.join(workDir, filePath);

          if (!fileExists(fullPath)) {
            return errorResult(`File ${filePath} does not exist`);
          }

          const content = readFileSync(fullPath);

          return result({
            success: true,
            path: filePath,
            content,
            size: content.length,
          });
        }
      ),

      tool(
        'list_files',
        'List files in a directory',
        {
          directory: z.string().optional().describe('Directory path relative to project root'),
          recursive: z.boolean().optional().describe('Whether to list files recursively'),
        },
        async ({ directory = '.', recursive = false }) => {
          const fullPath = path.join(workDir, directory);

          if (!fs.existsSync(fullPath)) {
            return errorResult(`Directory ${directory} does not exist`);
          }

          const files = listFilesInDir(fullPath, recursive);

          return result({
            success: true,
            directory,
            files: files.map(f => path.relative(workDir, f)),
            count: files.length,
          });
        }
      ),

      tool(
        'run_command',
        'Run a shell command in the project directory (npm install, npm run dev, etc.)',
        {
          command: z.string().describe('Command to run'),
          background: z.boolean().optional().describe('Run in background'),
        },
        async ({ command, background = false }) => {
          if (!allowWrite) {
            return result({
              success: false,
              preview: true,
              message: `Would run: ${command}`,
            });
          }

          try {
            if (background) {
              const child = spawn(command, { shell: true, cwd: workDir, detached: true, stdio: 'ignore' });
              child.unref();
              return result({
                success: true,
                message: `Started in background: ${command}`,
                pid: child.pid,
              });
            }

            const output = execSync(command, { cwd: workDir, encoding: 'utf8', timeout: 120000 });
            return result({
              success: true,
              command,
              output: output.slice(0, 5000),
            });
          } catch (error) {
            return errorResult(error.message);
          }
        }
      ),

      tool(
        'seed_database',
        'Seed the commerce database with sample products and data',
        {
          dbPath: z.string().optional().describe('Path to database file'),
          productCount: z.number().optional().describe('Number of sample products to create'),
        },
        async ({ dbPath = './store.db', productCount = 10 }) => {
          if (!allowWrite) {
            return result({
              success: false,
              preview: true,
              message: `Would seed database at ${dbPath} with ${productCount} products`,
            });
          }

          const seedScript = generateSeedScript(dbPath, productCount);
          const seedPath = path.join(workDir, 'scripts/seed.js');

          writeFileSync(seedPath, seedScript);

          return result({
            success: true,
            message: `Created seed script at scripts/seed.js`,
            nextSteps: ['Run: node scripts/seed.js'],
          });
        }
      ),
    ],
  });
}

// ============================================================================
// File Generation Functions
// ============================================================================

function createPackageJson(name, template, features) {
  const base = {
    name: name.toLowerCase().replace(/[^a-z0-9-]/g, '-'),
    version: '0.5.0',
    private: true,
    scripts: {
      dev: 'next dev',
      build: 'next build',
      start: 'next start',
      lint: 'next lint',
      seed: 'node scripts/seed.js',
    },
    dependencies: {
      '@stateset/embedded': '^0.5.0',
      next: '14.0.0',
      react: '^18',
      'react-dom': '^18',
    },
    devDependencies: {
      '@types/node': '^20',
      '@types/react': '^18',
      '@types/react-dom': '^18',
      typescript: '^5',
    },
  };

  if (TEMPLATES[template]?.features.includes('tailwind') || features.includes('tailwind')) {
    base.devDependencies.autoprefixer = '^10';
    base.devDependencies.postcss = '^8';
    base.devDependencies.tailwindcss = '^3';
  }

  return base;
}

function createTsConfig(template) {
  return JSON.stringify({
    compilerOptions: {
      target: 'es5',
      lib: ['dom', 'dom.iterable', 'esnext'],
      allowJs: true,
      skipLibCheck: true,
      strict: true,
      noEmit: true,
      esModuleInterop: true,
      module: 'esnext',
      moduleResolution: 'bundler',
      resolveJsonModule: true,
      isolatedModules: true,
      jsx: 'preserve',
      incremental: true,
      plugins: [{ name: 'next' }],
      paths: { '@/*': ['./*'] },
    },
    include: ['next-env.d.ts', '**/*.ts', '**/*.tsx', '.next/types/**/*.ts'],
    exclude: ['node_modules'],
  }, null, 2);
}

function createNextConfig() {
  return `/** @type {import('next').NextConfig} */
const nextConfig = {
  images: {
    remotePatterns: [
      { protocol: 'https', hostname: 'images.unsplash.com' },
    ],
  },
  experimental: {
    serverComponentsExternalPackages: ['@stateset/embedded'],
  },
};

module.exports = nextConfig;
`;
}

function createTailwindConfig() {
  return `import type { Config } from 'tailwindcss';

const config: Config = {
  content: [
    './pages/**/*.{js,ts,jsx,tsx,mdx}',
    './components/**/*.{js,ts,jsx,tsx,mdx}',
    './app/**/*.{js,ts,jsx,tsx,mdx}',
  ],
  theme: {
    extend: {},
  },
  plugins: [],
};

export default config;
`;
}

function createPostCssConfig() {
  return `module.exports = {
  plugins: {
    tailwindcss: {},
    autoprefixer: {},
  },
};
`;
}

function createCommerceLib() {
  return `import { Commerce } from '@stateset/embedded';

let commerce: Commerce | null = null;

export function getCommerce(): Commerce {
  if (!commerce) {
    commerce = new Commerce(process.env.DATABASE_PATH || './store.db');
  }
  return commerce;
}

export async function getProducts(options?: { limit?: number; offset?: number }) {
  const commerce = getCommerce();
  return commerce.products.list(options);
}

export async function getProduct(id: string) {
  const commerce = getCommerce();
  return commerce.products.get(id);
}

export async function getProductBySlug(slug: string) {
  const commerce = getCommerce();
  const products = await commerce.products.list();
  return products.products?.find(p => p.slug === slug);
}
`;
}

function createRootLayout(name) {
  return `import type { Metadata } from 'next';
import { Inter } from 'next/font/google';
import './globals.css';

const inter = Inter({ subsets: ['latin'] });

export const metadata: Metadata = {
  title: '${name}',
  description: 'Powered by StateSet iCommerce',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body className={inter.className}>
        <header className="border-b">
          <nav className="container mx-auto px-4 py-4 flex justify-between items-center">
            <a href="/" className="text-xl font-bold">${name}</a>
            <div className="flex gap-4">
              <a href="/products" className="hover:underline">Products</a>
              <a href="/cart" className="hover:underline">Cart</a>
            </div>
          </nav>
        </header>
        <main>{children}</main>
        <footer className="border-t mt-16">
          <div className="container mx-auto px-4 py-8 text-center text-gray-500">
            Powered by StateSet iCommerce
          </div>
        </footer>
      </body>
    </html>
  );
}
`;
}

function createHomePage() {
  return `import { getProducts } from '@/lib/commerce';
import Link from 'next/link';

export default async function HomePage() {
  const { products } = await getProducts({ limit: 8 });

  return (
    <div className="container mx-auto px-4 py-16">
      <section className="text-center mb-16">
        <h1 className="text-4xl font-bold mb-4">Welcome to Our Store</h1>
        <p className="text-gray-600 mb-8">Discover our amazing products</p>
        <Link
          href="/products"
          className="bg-black text-white px-6 py-3 rounded-lg hover:bg-gray-800"
        >
          Shop Now
        </Link>
      </section>

      <section>
        <h2 className="text-2xl font-bold mb-8">Featured Products</h2>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
          {products?.map((product) => (
            <Link
              key={product.id}
              href={\`/products/\${product.slug || product.id}\`}
              className="group"
            >
              <div className="aspect-square bg-gray-100 rounded-lg mb-4" />
              <h3 className="font-medium group-hover:underline">{product.name}</h3>
              <p className="text-gray-600">
                {product.variants?.[0]?.price ? \`$\${product.variants[0].price.toFixed(2)}\` : 'Price TBD'}
              </p>
            </Link>
          ))}
        </div>
      </section>
    </div>
  );
}
`;
}

function createGlobalStyles() {
  return `@tailwind base;
@tailwind components;
@tailwind utilities;

:root {
  --foreground-rgb: 0, 0, 0;
  --background-rgb: 255, 255, 255;
}

body {
  color: rgb(var(--foreground-rgb));
  background: rgb(var(--background-rgb));
}
`;
}

function createGitignore() {
  return `# Dependencies
node_modules/
.pnp
.pnp.js

# Next.js
.next/
out/

# Production
build/
dist/

# Database
*.db
*.sqlite
*.sqlite3

# Environment
.env
.env.local
.env.development.local
.env.test.local
.env.production.local

# Debug
npm-debug.log*
yarn-debug.log*
yarn-error.log*

# IDE
.vscode/
.idea/
*.swp
*.swo

# OS
.DS_Store
Thumbs.db
`;
}

function createEnvLocal() {
  return `# Database
DATABASE_PATH=./store.db

# Add your environment variables here
`;
}

function createReadme(name, template) {
  return `# ${name}

A commerce storefront built with StateSet iCommerce engine.

## Getting Started

1. Install dependencies:
   \`\`\`bash
   npm install
   \`\`\`

2. Seed the database (optional):
   \`\`\`bash
   npm run seed
   \`\`\`

3. Start the development server:
   \`\`\`bash
   npm run dev
   \`\`\`

4. Open [http://localhost:3000](http://localhost:3000)

## Stack

- **Framework:** ${TEMPLATES[template]?.name || 'Next.js'}
- **Commerce:** StateSet iCommerce (@stateset/embedded)
- **Database:** SQLite (embedded)
- **Styling:** Tailwind CSS

## Project Structure

\`\`\`
${name}/
├── app/                  # Next.js App Router
│   ├── products/        # Product pages
│   ├── cart/            # Shopping cart
│   ├── checkout/        # Checkout flow
│   └── api/             # API routes
├── components/          # React components
│   ├── commerce/        # Commerce components
│   └── ui/              # UI components
├── lib/                 # Utilities
│   └── commerce.ts      # StateSet client
├── hooks/               # React hooks
└── store.db             # SQLite database
\`\`\`

## Learn More

- [StateSet Documentation](https://docs.stateset.io)
- [Next.js Documentation](https://nextjs.org/docs)
`;
}

function generatePageContent(pageType, customName) {
  const templates = {
    'product-listing': `import { getProducts } from '@/lib/commerce';

export default async function ProductsPage() {
  const { products } = await getProducts();

  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-3xl font-bold mb-8">All Products</h1>
      <div className="grid grid-cols-1 md:grid-cols-3 lg:grid-cols-4 gap-6">
        {products?.map((product) => (
          <a
            key={product.id}
            href={\`/products/\${product.slug || product.id}\`}
            className="group"
          >
            <div className="aspect-square bg-gray-100 rounded-lg mb-4" />
            <h3 className="font-medium group-hover:underline">{product.name}</h3>
          </a>
        ))}
      </div>
    </div>
  );
}
`,
    'product-detail': `import { getProductBySlug } from '@/lib/commerce';
import { notFound } from 'next/navigation';

interface Props {
  params: { slug: string };
}

export default async function ProductPage({ params }: Props) {
  const product = await getProductBySlug(params.slug);

  if (!product) {
    notFound();
  }

  return (
    <div className="container mx-auto px-4 py-8">
      <div className="grid md:grid-cols-2 gap-8">
        <div className="aspect-square bg-gray-100 rounded-lg" />
        <div>
          <h1 className="text-3xl font-bold">{product.name}</h1>
          <p className="text-2xl font-semibold mt-4">
            {product.variants?.[0]?.price?.toFixed(2) || 'Price TBD'}
          </p>
          <p className="mt-4 text-gray-600">{product.description}</p>
          <button className="mt-6 bg-black text-white px-6 py-3 rounded-lg hover:bg-gray-800">
            Add to Cart
          </button>
        </div>
      </div>
    </div>
  );
}
`,
    'cart': `'use client';

export default function CartPage() {
  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-3xl font-bold mb-8">Shopping Cart</h1>
      <p className="text-gray-600">Your cart is empty.</p>
    </div>
  );
}
`,
    'checkout': `'use client';

export default function CheckoutPage() {
  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-3xl font-bold mb-8">Checkout</h1>
      {/* Checkout form */}
    </div>
  );
}
`,
    'account': `export default function AccountPage() {
  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-3xl font-bold mb-8">My Account</h1>
    </div>
  );
}
`,
    'orders': `export default function OrdersPage() {
  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-3xl font-bold mb-8">Order History</h1>
    </div>
  );
}
`,
  };

  return templates[pageType] || `export default function ${customName || 'Page'}() {
  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-3xl font-bold mb-8">${customName || 'Page'}</h1>
    </div>
  );
}
`;
}

function generateComponentContent(componentType, customName) {
  const templates = {
    'product-card': `import Link from 'next/link';

interface ProductCardProps {
  product: {
    id: string;
    name: string;
    slug?: string;
    imageUrl?: string;
    variants?: Array<{ price?: number }>;
  };
}

export function ProductCard({ product }: ProductCardProps) {
  const price = product.variants?.[0]?.price;

  return (
    <Link href={\`/products/\${product.slug || product.id}\`} className="group">
      <div className="aspect-square bg-gray-100 rounded-lg overflow-hidden mb-4">
        {product.imageUrl && (
          <img
            src={product.imageUrl}
            alt={product.name}
            className="w-full h-full object-cover group-hover:scale-105 transition-transform"
          />
        )}
      </div>
      <h3 className="font-medium group-hover:underline">{product.name}</h3>
      {price && <p className="text-gray-600">\${price.toFixed(2)}</p>}
    </Link>
  );
}
`,
    'add-to-cart': `'use client';

import { useState } from 'react';

interface AddToCartButtonProps {
  productId: string;
  variantId?: string;
  className?: string;
}

export function AddToCartButton({ productId, variantId, className }: AddToCartButtonProps) {
  const [isAdding, setIsAdding] = useState(false);

  const handleClick = async () => {
    setIsAdding(true);
    try {
      // Add to cart logic here
      await new Promise(resolve => setTimeout(resolve, 500));
    } finally {
      setIsAdding(false);
    }
  };

  return (
    <button
      onClick={handleClick}
      disabled={isAdding}
      className={\`bg-black text-white px-6 py-3 rounded-lg hover:bg-gray-800 disabled:opacity-50 \${className}\`}
    >
      {isAdding ? 'Adding...' : 'Add to Cart'}
    </button>
  );
}
`,
    'header': `import Link from 'next/link';

export function Header() {
  return (
    <header className="border-b">
      <nav className="container mx-auto px-4 py-4 flex justify-between items-center">
        <Link href="/" className="text-xl font-bold">Store</Link>
        <div className="flex gap-4">
          <Link href="/products" className="hover:underline">Products</Link>
          <Link href="/cart" className="hover:underline">Cart</Link>
        </div>
      </nav>
    </header>
  );
}
`,
    'footer': `export function Footer() {
  return (
    <footer className="border-t mt-16">
      <div className="container mx-auto px-4 py-8 text-center text-gray-500">
        <p>Powered by StateSet iCommerce</p>
      </div>
    </footer>
  );
}
`,
  };

  return templates[componentType] || `interface ${customName || 'Component'}Props {}

export function ${customName || 'Component'}({}: ${customName || 'Component'}Props) {
  return (
    <div>
      {/* ${customName || 'Component'} */}
    </div>
  );
}
`;
}

function generateHookContent(hookName, customName) {
  const templates = {
    'useCart': `'use client';

import { useState, useEffect, useCallback } from 'react';

const CART_ID_KEY = 'stateset_cart_id';

export function useCart() {
  const [cart, setCart] = useState(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const cartId = localStorage.getItem(CART_ID_KEY);
    if (cartId) {
      fetchCart(cartId);
    } else {
      setIsLoading(false);
    }
  }, []);

  const fetchCart = async (cartId: string) => {
    try {
      const res = await fetch(\`/api/carts/\${cartId}\`);
      if (res.ok) {
        const data = await res.json();
        setCart(data.cart);
      }
    } finally {
      setIsLoading(false);
    }
  };

  const addItem = useCallback(async (item: any) => {
    // Implementation
  }, [cart]);

  const removeItem = useCallback(async (itemId: string) => {
    // Implementation
  }, [cart]);

  return {
    cart,
    isLoading,
    itemCount: cart?.items?.length || 0,
    addItem,
    removeItem,
  };
}
`,
    'useProducts': `'use client';

import { useState, useEffect } from 'react';

export function useProducts(options?: { limit?: number }) {
  const [products, setProducts] = useState([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState(null);

  useEffect(() => {
    const fetchProducts = async () => {
      try {
        const res = await fetch('/api/products');
        const data = await res.json();
        setProducts(data.products || []);
      } catch (err) {
        setError(err);
      } finally {
        setIsLoading(false);
      }
    };

    fetchProducts();
  }, [options?.limit]);

  return { products, isLoading, error };
}
`,
  };

  return templates[hookName] || `'use client';

import { useState } from 'react';

export function ${customName || hookName}() {
  const [state, setState] = useState(null);

  return { state };
}
`;
}

function generateApiRouteContent(routePath, methods) {
  const handlers = methods.map(method => {
    return `export async function ${method}(request: NextRequest) {
  const commerce = getCommerce();

  try {
    // TODO: Implement ${method} handler for ${routePath}
    return NextResponse.json({ success: true });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Unknown error' },
      { status: 500 }
    );
  }
}`;
  }).join('\n\n');

  return `import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';

${handlers}
`;
}

function generateSeedScript(dbPath, productCount) {
  return `const { Commerce } = require('@stateset/embedded');

async function seed() {
  const commerce = new Commerce('${dbPath}');

  console.log('Seeding database...');

  const products = [
    { name: 'Classic T-Shirt', slug: 'classic-t-shirt', price: 29.99 },
    { name: 'Premium Hoodie', slug: 'premium-hoodie', price: 79.99 },
    { name: 'Canvas Sneakers', slug: 'canvas-sneakers', price: 59.99 },
    { name: 'Leather Wallet', slug: 'leather-wallet', price: 49.99 },
    { name: 'Wireless Earbuds', slug: 'wireless-earbuds', price: 99.99 },
    { name: 'Smart Watch', slug: 'smart-watch', price: 199.99 },
    { name: 'Backpack', slug: 'backpack', price: 89.99 },
    { name: 'Sunglasses', slug: 'sunglasses', price: 149.99 },
    { name: 'Water Bottle', slug: 'water-bottle', price: 24.99 },
    { name: 'Notebook Set', slug: 'notebook-set', price: 19.99 },
  ].slice(0, ${productCount});

  for (const product of products) {
    try {
      await commerce.products.create({
        name: product.name,
        slug: product.slug,
        description: \`High-quality \${product.name.toLowerCase()}\`,
        variants: [
          {
            sku: product.slug.toUpperCase(),
            name: 'Default',
            price: product.price,
            isDefault: true,
          },
        ],
      });

      await commerce.inventory.createItem({
        sku: product.slug.toUpperCase(),
        name: product.name,
        initialQuantity: 100,
      });

      console.log(\`Created: \${product.name}\`);
    } catch (err) {
      console.error(\`Error creating \${product.name}:\`, err.message);
    }
  }

  console.log('Seeding complete!');
}

seed().catch(console.error);
`;
}

function listFilesInDir(dir, recursive = false) {
  const files = [];
  const entries = fs.readdirSync(dir, { withFileTypes: true });

  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (recursive && !entry.name.startsWith('.') && entry.name !== 'node_modules') {
        files.push(...listFilesInDir(fullPath, true));
      }
    } else {
      files.push(fullPath);
    }
  }

  return files;
}

// ============================================================================
// Tool Names Export
// ============================================================================

export const SCAFFOLD_TOOL_NAMES = [
  'list_templates',
  'list_page_templates',
  'list_component_templates',
  'create_project',
  'add_page',
  'add_component',
  'add_hook',
  'add_api_route',
  'write_file',
  'read_file',
  'list_files',
  'run_command',
  'seed_database',
];

export default createScaffoldMcpServer;
