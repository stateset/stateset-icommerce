const assert = require('node:assert/strict');
const { test } = require('node:test');
const { readFileSync } = require('node:fs');
const vm = require('node:vm');
const { loadNativeBinding } = require('../native-loader.js');
const manifest = require('../package.json');

const missing = () =>
  Object.assign(new Error("Cannot find module '@stateset/embedded-linux-x64-gnu'"), {
    code: 'MODULE_NOT_FOUND',
  });

test('public entry point decorates an actual generated-loader failure', () => {
  const cause = missing();
  const code = readFileSync(require.resolve('../index.js'), 'utf8');
  assert.throws(
    () =>
      vm.runInNewContext(code, {
        require(name) {
          if (name === './native-binding.js') throw cause;
          return require(`../${name.replace(/^\.\//, '')}`);
        },
      }),
    (error) => {
      assert.equal(error.code, 'STATESET_NATIVE_BINDING_LOAD_FAILED');
      assert.equal(error.cause, cause);
      assert.match(error.message, /--include=optional/);
      return true;
    },
  );
});

test('missing optional binary explains unsupported Node versions and preserves its cause', () => {
  for (const version of ['18.20.8', '20.19.2', '20.20.0-rc.1']) {
    const cause = missing();
    assert.throws(
      () =>
        loadNativeBinding(() => {
          throw cause;
        }, version),
      (error) => {
        assert.equal(error.code, 'STATESET_UNSUPPORTED_NODE_VERSION');
        assert.equal(error.cause, cause);
        assert.ok(error.message.includes(manifest.engines.node));
        assert.ok(error.message.includes(version));
        assert.match(error.message, /npm.*skipped/i);
        return true;
      },
    );
  }
});

test('supported runtimes get platform package and optional dependency recovery guidance', () => {
  for (const version of ['20.20.0', '20.20.1', '22.20.0', '24.0.0']) {
    const cause = missing();
    assert.throws(
      () =>
        loadNativeBinding(() => {
          throw cause;
        }, version),
      (error) => {
        assert.equal(error.code, 'STATESET_NATIVE_BINDING_LOAD_FAILED');
        assert.equal(error.cause, cause);
        assert.match(error.message, /@stateset\/embedded-linux-x64-gnu/);
        assert.match(error.message, /--include=optional/);
        assert.ok(error.message.includes(`@stateset/embedded@${manifest.version}`));
        assert.doesNotMatch(error.message, /UNSUPPORTED_NODE_VERSION/);
        return true;
      },
    );
  }
});

test('ABI and libc failures retain the original diagnostic', () => {
  const cause = Object.assign(new Error('GLIBC_2.38 not found'), { code: 'ERR_DLOPEN_FAILED' });
  assert.throws(
    () =>
      loadNativeBinding(() => {
        throw cause;
      }, '22.20.0'),
    (error) => {
      assert.equal(error.code, 'STATESET_NATIVE_BINDING_LOAD_FAILED');
      assert.equal(error.cause, cause);
      assert.match(error.message, /GLIBC_2.38 not found/);
      assert.match(error.message, /libc/);
      return true;
    },
  );
});

test('successful loads and unrelated errors pass through unchanged', () => {
  const binding = { Commerce: class {} };
  assert.equal(
    loadNativeBinding(() => binding, '22.20.0'),
    binding,
  );
  const cause = new Error('Unsupported OS: freebsd');
  assert.throws(
    () =>
      loadNativeBinding(() => {
        throw cause;
      }, '22.20.0'),
    (error) => error === cause,
  );
});
