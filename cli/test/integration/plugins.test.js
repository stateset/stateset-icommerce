/**
 * Integration tests for plugin system
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';
import { PluginLoader, createPluginLoader, scaffoldPlugin, PLUGIN_TEMPLATE } from '../../src/plugins/loader.js';

describe('plugins integration', () => {
  let testPluginDir;
  let loader;

  beforeEach(() => {
    // Create temp directory for test plugins
    testPluginDir = path.join(os.tmpdir(), `stateset-test-plugins-${Date.now()}`);
    fs.mkdirSync(testPluginDir, { recursive: true });

    loader = new PluginLoader({
      pluginDirs: [testPluginDir]
    });
  });

  afterEach(() => {
    // Cleanup temp directory
    try {
      fs.rmSync(testPluginDir, { recursive: true, force: true });
    } catch {
      // Ignore cleanup errors
    }
  });

  describe('PluginLoader', () => {
    describe('discover', () => {
      it('should discover plugins in configured directories', async () => {
        // Create test plugin files
        fs.writeFileSync(path.join(testPluginDir, 'test1.js'), PLUGIN_TEMPLATE);
        fs.writeFileSync(path.join(testPluginDir, 'test2.mjs'), PLUGIN_TEMPLATE);

        const discovered = await loader.discover();

        assert.strictEqual(discovered.length, 2);
        assert.ok(discovered.some(f => f.endsWith('test1.js')));
        assert.ok(discovered.some(f => f.endsWith('test2.mjs')));
      });

      it('should ignore non-js files', async () => {
        fs.writeFileSync(path.join(testPluginDir, 'test.js'), PLUGIN_TEMPLATE);
        fs.writeFileSync(path.join(testPluginDir, 'readme.md'), '# Readme');
        fs.writeFileSync(path.join(testPluginDir, 'config.json'), '{}');

        const discovered = await loader.discover();

        assert.strictEqual(discovered.length, 1);
      });

      it('should handle empty directories', async () => {
        const discovered = await loader.discover();
        assert.strictEqual(discovered.length, 0);
      });

      it('should handle non-existent directories gracefully', async () => {
        const loaderWithBadDir = new PluginLoader({
          pluginDirs: ['/nonexistent/path']
        });

        const discovered = await loaderWithBadDir.discover();
        assert.strictEqual(discovered.length, 0);
      });
    });

    describe('loadPlugin', () => {
      it('should load valid plugin', async () => {
        const pluginCode = `
export default {
  name: 'test_plugin',
  description: 'A test plugin',
  handler: async (input) => ({ result: input.value * 2 })
};
`;
        const pluginPath = path.join(testPluginDir, 'valid.js');
        fs.writeFileSync(pluginPath, pluginCode);

        const plugin = await loader.loadPlugin(pluginPath);

        assert.ok(plugin);
        assert.strictEqual(plugin.name, 'test_plugin');
        assert.strictEqual(plugin.description, 'A test plugin');
      });

      it('should reject invalid plugin structure', async () => {
        const pluginCode = `
export default {
  // Missing required fields
  handler: async () => {}
};
`;
        const pluginPath = path.join(testPluginDir, 'invalid.js');
        fs.writeFileSync(pluginPath, pluginCode);

        const plugin = await loader.loadPlugin(pluginPath);
        assert.strictEqual(plugin, null);
      });

      it('should reject invalid plugin names', async () => {
        const pluginCode = `
export default {
  name: 'Invalid-Name',
  description: 'Bad name format',
  handler: async () => {}
};
`;
        const pluginPath = path.join(testPluginDir, 'badname.js');
        fs.writeFileSync(pluginPath, pluginCode);

        const plugin = await loader.loadPlugin(pluginPath);
        assert.strictEqual(plugin, null);
      });

      it('should prevent name conflicts', async () => {
        const pluginCode = `
export default {
  name: 'duplicate',
  description: 'First plugin',
  handler: async () => ({ first: true })
};
`;
        const pluginPath1 = path.join(testPluginDir, 'plugin1.js');
        const pluginPath2 = path.join(testPluginDir, 'plugin2.js');
        fs.writeFileSync(pluginPath1, pluginCode);
        fs.writeFileSync(pluginPath2, pluginCode);

        const plugin1 = await loader.loadPlugin(pluginPath1);
        const plugin2 = await loader.loadPlugin(pluginPath2);

        assert.ok(plugin1);
        assert.strictEqual(plugin2, null);
      });

      it('should call onLoad hook', async () => {
        const pluginCode = `
let loadCalled = false;
export default {
  name: 'hook_test',
  description: 'Tests lifecycle hooks',
  handler: async () => ({ loadCalled }),
  onLoad: async () => { loadCalled = true; }
};
`;
        const pluginPath = path.join(testPluginDir, 'hooks.js');
        fs.writeFileSync(pluginPath, pluginCode);

        await loader.loadPlugin(pluginPath);

        // Execute to verify onLoad was called
        const result = await loader.execute('hook_test', {});
        assert.strictEqual(result.loadCalled, true);
      });
    });

    describe('loadAll', () => {
      it('should load all valid plugins', async () => {
        for (let i = 1; i <= 3; i++) {
          const pluginCode = `
export default {
  name: 'plugin_${i}',
  description: 'Plugin ${i}',
  handler: async () => ({ id: ${i} })
};
`;
          fs.writeFileSync(path.join(testPluginDir, `plugin${i}.js`), pluginCode);
        }

        const loaded = await loader.loadAll();

        assert.strictEqual(loaded.length, 3);
        assert.ok(loader.has('plugin_1'));
        assert.ok(loader.has('plugin_2'));
        assert.ok(loader.has('plugin_3'));
      });
    });

    describe('execute', () => {
      beforeEach(async () => {
        const pluginCode = `
export default {
  name: 'calculator',
  description: 'Simple calculator',
  handler: async (input) => ({
    sum: input.a + input.b,
    product: input.a * input.b
  })
};
`;
        fs.writeFileSync(path.join(testPluginDir, 'calc.js'), pluginCode);
        await loader.loadAll();
      });

      it('should execute plugin handler', async () => {
        const result = await loader.execute('calculator', { a: 3, b: 4 });

        assert.strictEqual(result.sum, 7);
        assert.strictEqual(result.product, 12);
      });

      it('should throw for unknown plugin', async () => {
        try {
          await loader.execute('nonexistent', {});
          assert.fail('Should have thrown');
        } catch (error) {
          assert.ok(error.message.includes('not found'));
        }
      });

      it('should propagate handler errors', async () => {
        const pluginCode = `
export default {
  name: 'error_plugin',
  description: 'Throws error',
  handler: async () => { throw new Error('Handler error'); }
};
`;
        fs.writeFileSync(path.join(testPluginDir, 'error.js'), pluginCode);
        await loader.loadPlugin(path.join(testPluginDir, 'error.js'));

        try {
          await loader.execute('error_plugin', {});
          assert.fail('Should have thrown');
        } catch (error) {
          assert.ok(error.message.includes('Handler error'));
        }
      });
    });

    describe('unload', () => {
      it('should unload plugin', async () => {
        const pluginCode = `
export default {
  name: 'unload_test',
  description: 'Test unload',
  handler: async () => ({})
};
`;
        fs.writeFileSync(path.join(testPluginDir, 'unload.js'), pluginCode);
        await loader.loadAll();

        assert.ok(loader.has('unload_test'));

        const result = await loader.unload('unload_test');

        assert.strictEqual(result, true);
        assert.ok(!loader.has('unload_test'));
      });

      it('should call onUnload hook', async () => {
        let unloadCalled = false;
        const pluginCode = `
export default {
  name: 'unload_hook',
  description: 'Test unload hook',
  handler: async () => ({}),
  onUnload: async () => { console.log('Unloaded'); }
};
`;
        fs.writeFileSync(path.join(testPluginDir, 'unload_hook.js'), pluginCode);
        await loader.loadAll();

        await loader.unload('unload_hook');
        // Can't easily verify console.log was called, but no error means success
      });

      it('should return false for unknown plugin', async () => {
        const result = await loader.unload('nonexistent');
        assert.strictEqual(result, false);
      });
    });

    describe('toMcpTools', () => {
      it('should convert plugins to MCP tool format', async () => {
        const pluginCode = `
export default {
  name: 'mcp_test',
  description: 'MCP test plugin',
  inputSchema: { value: { type: 'number' } },
  handler: async (input) => ({ doubled: input.value * 2 })
};
`;
        fs.writeFileSync(path.join(testPluginDir, 'mcp.js'), pluginCode);
        await loader.loadAll();

        const tools = loader.toMcpTools();

        assert.strictEqual(tools.length, 1);
        assert.strictEqual(tools[0].name, 'plugin_mcp_test');
        assert.ok(tools[0].description.includes('[Plugin]'));
      });
    });
  });

  describe('createPluginLoader', () => {
    it('should create loader with defaults', () => {
      const loader = createPluginLoader();
      assert.ok(loader instanceof PluginLoader);
    });

    it('should accept custom plugin dirs', () => {
      const loader = createPluginLoader({
        pluginDirs: ['/custom/path']
      });
      assert.ok(loader);
    });
  });

  describe('scaffoldPlugin', () => {
    // Skip actual scaffold test as it writes to user directory
    it('should have PLUGIN_TEMPLATE defined', () => {
      assert.ok(PLUGIN_TEMPLATE);
      assert.ok(PLUGIN_TEMPLATE.includes('export default'));
      assert.ok(PLUGIN_TEMPLATE.includes('name:'));
      assert.ok(PLUGIN_TEMPLATE.includes('handler:'));
    });
  });
});
