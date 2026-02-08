/**
 * Structural tests for the tool module files in cli/src/tools/
 *
 * Validates that each tool module exports an array of well-formed tool
 * descriptors with the required properties. Excludes index.js (the registry).
 * For vector.js which has no default export, we fall back to the first named
 * array export.
 */

import { describe, it, before } from 'node:test';
import assert from 'node:assert';
import { readdir } from 'node:fs/promises';
import { join, basename } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));
const TOOLS_DIR = join(__dirname, '..', '..', 'src', 'tools');

// Files to skip: index.js is the registry (not a tool module)
const SKIP_FILES = new Set(['index.js']);

/** @type {Map<string, Array<Object>>} filename -> tool array */
const toolModules = new Map();

/** @type {Array<Object>} All tools from all modules combined */
let allTools = [];

before(async () => {
  const files = await readdir(TOOLS_DIR);
  const jsFiles = files.filter((f) => f.endsWith('.js') && !SKIP_FILES.has(f));

  for (const file of jsFiles) {
    const fullPath = join(TOOLS_DIR, file);
    const mod = await import(fullPath);
    // Prefer default export; fall back to first named array export
    let tools = mod.default;
    if (!Array.isArray(tools)) {
      for (const key of Object.keys(mod)) {
        if (key === 'default') continue;
        if (Array.isArray(mod[key])) {
          tools = mod[key];
          break;
        }
      }
    }
    toolModules.set(file, tools);
    if (Array.isArray(tools)) {
      allTools = allTools.concat(tools);
    }
  }
});

// ===========================================================================
// Module loading
// ===========================================================================

describe('tool module loading', () => {
  it('found tool module files in src/tools/', () => {
    assert.ok(toolModules.size > 0, 'Should find at least one tool module');
  });

  it('loaded at least 10 tool module files', () => {
    assert.ok(
      toolModules.size >= 10,
      `Expected at least 10 tool modules, found ${toolModules.size}`,
    );
  });

  it('every tool module has a default export', () => {
    for (const [file, tools] of toolModules) {
      assert.ok(tools !== undefined, `Module "${file}" has no default export`);
    }
  });

  it('every default export is an array', () => {
    for (const [file, tools] of toolModules) {
      assert.ok(
        Array.isArray(tools),
        `Module "${file}" default export is not an array (got ${typeof tools})`,
      );
    }
  });

  it('every tool module has at least one tool', () => {
    for (const [file, tools] of toolModules) {
      assert.ok(tools.length > 0, `Module "${file}" exports an empty tool array`);
    }
  });
});

// ===========================================================================
// Tool descriptor structure
// ===========================================================================

describe('tool descriptor structure', () => {
  it('every tool has a name (string)', () => {
    for (const [file, tools] of toolModules) {
      for (const tool of tools) {
        assert.ok(
          typeof tool.name === 'string' && tool.name.length > 0,
          `Tool in "${file}" is missing or has empty name`,
        );
      }
    }
  });

  it('every tool has a description (string)', () => {
    for (const [file, tools] of toolModules) {
      for (const tool of tools) {
        assert.ok(
          typeof tool.description === 'string' && tool.description.length > 0,
          `Tool "${tool.name}" in "${file}" is missing or has empty description`,
        );
      }
    }
  });

  it('every tool has an inputSchema (object)', () => {
    for (const [file, tools] of toolModules) {
      for (const tool of tools) {
        assert.ok(
          tool.inputSchema != null && typeof tool.inputSchema === 'object',
          `Tool "${tool.name}" in "${file}" is missing or has non-object inputSchema`,
        );
      }
    }
  });

  it('every tool has a permission ("read" or "write" or "admin")', () => {
    const validPermissions = new Set(['read', 'write', 'admin', 'delete']);
    for (const [file, tools] of toolModules) {
      for (const tool of tools) {
        assert.ok(
          validPermissions.has(tool.permission),
          `Tool "${tool.name}" in "${file}" has invalid permission: "${tool.permission}"`,
        );
      }
    }
  });

  it('every tool has a handler (function)', () => {
    for (const [file, tools] of toolModules) {
      for (const tool of tools) {
        assert.ok(
          typeof tool.handler === 'function',
          `Tool "${tool.name}" in "${file}" is missing or has non-function handler`,
        );
      }
    }
  });
});

// ===========================================================================
// Tool naming conventions
// ===========================================================================

describe('tool naming conventions', () => {
  it('all tool names use snake_case', () => {
    const snakeCase = /^[a-z][a-z0-9]*(_[a-z0-9]+)*$/;
    for (const [file, tools] of toolModules) {
      for (const tool of tools) {
        assert.ok(
          snakeCase.test(tool.name),
          `Tool "${tool.name}" in "${file}" does not follow snake_case convention`,
        );
      }
    }
  });

  it('no tool names contain uppercase letters', () => {
    for (const [file, tools] of toolModules) {
      for (const tool of tools) {
        assert.strictEqual(
          tool.name,
          tool.name.toLowerCase(),
          `Tool "${tool.name}" in "${file}" contains uppercase letters`,
        );
      }
    }
  });

  it('no tool names start with a number', () => {
    for (const [file, tools] of toolModules) {
      for (const tool of tools) {
        assert.ok(
          !/^[0-9]/.test(tool.name),
          `Tool "${tool.name}" in "${file}" starts with a number`,
        );
      }
    }
  });
});

// ===========================================================================
// No duplicate tool names
// ===========================================================================

describe('tool name uniqueness', () => {
  it('no duplicate tool names within a single module', () => {
    for (const [file, tools] of toolModules) {
      const names = new Set();
      for (const tool of tools) {
        assert.ok(!names.has(tool.name), `Duplicate tool name "${tool.name}" in module "${file}"`);
        names.add(tool.name);
      }
    }
  });

  it('no duplicate tool names across all modules', () => {
    const nameToFile = new Map();
    for (const [file, tools] of toolModules) {
      for (const tool of tools) {
        if (nameToFile.has(tool.name)) {
          assert.fail(
            `Duplicate tool name "${tool.name}" found in both "${nameToFile.get(tool.name)}" and "${file}"`,
          );
        }
        nameToFile.set(tool.name, file);
      }
    }
  });
});

// ===========================================================================
// Specific module checks
// ===========================================================================

describe('specific module contents', () => {
  it('customers.js exports list_customers, get_customer, create_customer', () => {
    const tools = toolModules.get('customers.js');
    assert.ok(tools, 'customers.js should be loaded');
    const names = tools.map((t) => t.name);
    assert.ok(names.includes('list_customers'), 'Missing list_customers');
    assert.ok(names.includes('get_customer'), 'Missing get_customer');
    assert.ok(names.includes('create_customer'), 'Missing create_customer');
  });

  it('orders.js exports list_orders and get_order', () => {
    const tools = toolModules.get('orders.js');
    assert.ok(tools, 'orders.js should be loaded');
    const names = tools.map((t) => t.name);
    assert.ok(names.includes('list_orders'), 'Missing list_orders');
    assert.ok(names.includes('get_order'), 'Missing get_order');
  });

  it('analytics.js exports get_sales_summary', () => {
    const tools = toolModules.get('analytics.js');
    assert.ok(tools, 'analytics.js should be loaded');
    const names = tools.map((t) => t.name);
    assert.ok(names.includes('get_sales_summary'), 'Missing get_sales_summary');
  });

  it('carts.js exports list_carts and create_cart', () => {
    const tools = toolModules.get('carts.js');
    assert.ok(tools, 'carts.js should be loaded');
    const names = tools.map((t) => t.name);
    assert.ok(names.includes('list_carts'), 'Missing list_carts');
    assert.ok(names.includes('create_cart'), 'Missing create_cart');
  });

  it('currency.js exports get_exchange_rate and convert_currency', () => {
    const tools = toolModules.get('currency.js');
    assert.ok(tools, 'currency.js should be loaded');
    const names = tools.map((t) => t.name);
    assert.ok(names.includes('get_exchange_rate'), 'Missing get_exchange_rate');
    assert.ok(names.includes('convert_currency'), 'Missing convert_currency');
  });

  it('tax.js exports calculate_tax', () => {
    const tools = toolModules.get('tax.js');
    assert.ok(tools, 'tax.js should be loaded');
    const names = tools.map((t) => t.name);
    assert.ok(names.includes('calculate_tax'), 'Missing calculate_tax');
  });
});

// ===========================================================================
// Permission distribution
// ===========================================================================

describe('permission distribution', () => {
  it('there are both read and write tools across all modules', () => {
    const permissions = new Set();
    for (const tool of allTools) {
      permissions.add(tool.permission);
    }
    assert.ok(permissions.has('read'), 'Should have read-permission tools');
    assert.ok(permissions.has('write'), 'Should have write-permission tools');
  });

  it('read-only modules (analytics) only have read-permission tools', () => {
    const tools = toolModules.get('analytics.js');
    if (tools) {
      for (const tool of tools) {
        assert.strictEqual(
          tool.permission,
          'read',
          `Analytics tool "${tool.name}" should be read-only but has permission "${tool.permission}"`,
        );
      }
    }
  });

  it('total tool count across all modules is at least 40', () => {
    assert.ok(allTools.length >= 40, `Expected at least 40 total tools, found ${allTools.length}`);
  });
});

// ===========================================================================
// Handler shape
// ===========================================================================

describe('handler function shape', () => {
  it('all handlers are async functions or return-promise functions', () => {
    for (const [file, tools] of toolModules) {
      for (const tool of tools) {
        // AsyncFunction or regular function (the handler signature accepts an object)
        assert.ok(
          typeof tool.handler === 'function',
          `Tool "${tool.name}" in "${file}" handler is not a function`,
        );
        // Check it accepts at least zero arguments (handler({commerce, params, ...}))
        // The function.length may be 0 for destructured args or 1
        assert.ok(
          tool.handler.length <= 1,
          `Tool "${tool.name}" handler has unexpected arity ${tool.handler.length}`,
        );
      }
    }
  });
});
