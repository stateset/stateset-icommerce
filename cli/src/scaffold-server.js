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
import { spawn } from 'node:child_process';
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
} from './scaffold-templates.js';

// ============================================================================
// Security Helpers
// ============================================================================

/**
 * Resolve a user-supplied sub-path within a base directory, preventing
 * path traversal attacks. Throws if the resolved path escapes baseDir.
 * @param {string} baseDir - Trusted base directory
 * @param {string} subPath - User-supplied relative path
 * @returns {string} Resolved absolute path
 */
function safePath(baseDir, subPath) {
  const resolved = path.resolve(baseDir, subPath);
  const base = path.resolve(baseDir);
  if (!resolved.startsWith(base + path.sep) && resolved !== base) {
    throw new Error('Path traversal detected: path escapes the working directory');
  }
  return resolved;
}

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
    content: [{ type: 'text', text: JSON.stringify(data, null, 2) }],
  };
}

function errorResult(message) {
  return {
    content: [{ type: 'text', text: JSON.stringify({ success: false, error: message }, null, 2) }],
    isError: true,
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

      tool('list_templates', 'List available storefront project templates', {}, async () => {
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
      }),

      tool('list_page_templates', 'List available page templates', {}, async () => {
        return result({
          success: true,
          pages: Object.entries(PAGE_TEMPLATES).map(([id, p]) => ({
            id,
            name: p.name,
            description: p.description,
            path: p.path,
          })),
        });
      }),

      tool('list_component_templates', 'List available component templates', {}, async () => {
        return result({
          success: true,
          components: Object.entries(COMPONENT_TEMPLATES).map(([id, c]) => ({
            id,
            name: c.name,
            description: c.description,
            path: c.path,
          })),
        });
      }),

      tool(
        'create_project',
        'Create a new storefront project with the specified template. This initializes the full project structure.',
        {
          name: z.string().describe('Project name (used for directory and package name)'),
          template: z
            .enum(['nextjs', 'nextjs-minimal', 'vite-react', 'astro'])
            .describe('Project template to use'),
          directory: z
            .string()
            .optional()
            .describe('Directory to create project in (defaults to current directory)'),
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
          writeFileSync(
            path.join(projectDir, 'package.json'),
            JSON.stringify(packageJson, null, 2),
          );

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
            nextSteps: [`cd ${name}`, 'npm install', 'npm run dev'],
          });
        },
      ),

      tool(
        'add_page',
        'Add a page to the storefront project',
        {
          pageType: z
            .enum([
              'product-listing',
              'product-detail',
              'cart',
              'checkout',
              'account',
              'orders',
              'custom',
            ])
            .describe('Type of page to add'),
          customPath: z
            .string()
            .optional()
            .describe('Custom path for the page (only for custom type)'),
          customName: z
            .string()
            .optional()
            .describe('Custom name for the page (only for custom type)'),
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

          const fullPath = safePath(workDir, pagePath);
          const content = generatePageContent(pageType, customName);

          writeFileSync(fullPath, content);

          return result({
            success: true,
            message: `Created ${template?.name || customName} page`,
            path: pagePath,
          });
        },
      ),

      tool(
        'add_component',
        'Add a component to the storefront project',
        {
          componentType: z
            .enum([
              'product-card',
              'product-grid',
              'cart-drawer',
              'add-to-cart',
              'checkout-form',
              'header',
              'footer',
              'custom',
            ])
            .describe('Type of component to add'),
          customPath: z
            .string()
            .optional()
            .describe('Custom path for the component (only for custom type)'),
          customName: z
            .string()
            .optional()
            .describe('Custom name for the component (only for custom type)'),
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

          const fullPath = safePath(workDir, componentPath);
          const content = generateComponentContent(componentType, customName);

          writeFileSync(fullPath, content);

          return result({
            success: true,
            message: `Created ${template?.name || customName} component`,
            path: componentPath,
          });
        },
      ),

      tool(
        'add_hook',
        'Add a React hook to the storefront project',
        {
          hookName: z
            .enum(['useCart', 'useProducts', 'useCheckout', 'useCustomer', 'custom'])
            .describe('Name of the hook to add'),
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

          const fullPath = safePath(workDir, hookPath);
          const content = generateHookContent(hookName, customName);

          writeFileSync(fullPath, content);

          return result({
            success: true,
            message: `Created ${name} hook`,
            path: hookPath,
          });
        },
      ),

      tool(
        'add_api_route',
        'Add an API route to the storefront project',
        {
          routePath: z.string().describe('API route path (e.g., "products", "cart", "checkout")'),
          methods: z
            .array(z.enum(['GET', 'POST', 'PUT', 'PATCH', 'DELETE']))
            .describe('HTTP methods to support'),
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

          const fullPath = safePath(workDir, apiPath);
          const content = generateApiRouteContent(routePath, methods);

          writeFileSync(fullPath, content);

          return result({
            success: true,
            message: `Created API route at ${apiPath}`,
            path: apiPath,
            methods,
          });
        },
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
          const fullPath = safePath(workDir, filePath);

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
        },
      ),

      tool(
        'read_file',
        'Read content from a file in the project',
        {
          filePath: z.string().describe('Path to the file relative to project root'),
        },
        async ({ filePath }) => {
          const fullPath = safePath(workDir, filePath);

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
        },
      ),

      tool(
        'list_files',
        'List files in a directory',
        {
          directory: z.string().optional().describe('Directory path relative to project root'),
          recursive: z.boolean().optional().describe('Whether to list files recursively'),
        },
        async ({ directory = '.', recursive = false }) => {
          const fullPath = safePath(workDir, directory);

          if (!fs.existsSync(fullPath)) {
            return errorResult(`Directory ${directory} does not exist`);
          }

          const files = listFilesInDir(fullPath, recursive);

          return result({
            success: true,
            directory,
            files: files.map((f) => path.relative(workDir, f)),
            count: files.length,
          });
        },
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

          // Allowlist: only permit safe scaffold commands.
          // We match the executable exactly and reject shell metacharacters
          // to prevent command chaining (e.g. "npm install && rm -rf /").
          const ALLOWED_EXECUTABLES = new Set(['npm', 'npx', 'node', 'git', 'ls', 'cat', 'mkdir']);
          const SHELL_METACHAR_RE = /[;&|`$(){}!<>]/;
          const trimmed = command.trim();
          const executable = trimmed.split(/\s+/)[0];
          if (!ALLOWED_EXECUTABLES.has(executable)) {
            return errorResult(
              `Command not allowed. Permitted executables: ${[...ALLOWED_EXECUTABLES].join(', ')}`,
            );
          }
          if (SHELL_METACHAR_RE.test(trimmed)) {
            return errorResult(
              'Command contains disallowed shell metacharacters. Remove ;, &, |, `, $, etc.',
            );
          }

          try {
            // Use execFile (no shell) to prevent injection via metacharacters.
            const args = trimmed.split(/\s+/).slice(1);
            if (background) {
              const child = spawn(executable, args, {
                cwd: workDir,
                detached: true,
                stdio: 'ignore',
              });
              child.unref();
              return result({
                success: true,
                message: `Started in background: ${command}`,
                pid: child.pid,
              });
            }

            const { execFileSync } = await import('child_process');
            const output = execFileSync(executable, args, {
              cwd: workDir,
              encoding: 'utf8',
              timeout: 120000,
            });
            return result({
              success: true,
              command,
              output: output.slice(0, 5000),
            });
          } catch (error) {
            return errorResult(error.message);
          }
        },
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
        },
      ),
    ],
  });
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
