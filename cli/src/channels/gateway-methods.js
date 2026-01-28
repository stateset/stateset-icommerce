/**
 * Gateway Method Registry for StateSet iCommerce
 *
 * Allows plugins to register custom RPC-style methods that can be
 * invoked through the gateway. Methods are type-safe dispatched
 * with request/response patterns.
 *
 * Inspired by moltbot's gateway handler registration system.
 *
 * Usage:
 *   const methods = getGatewayMethods();
 *   methods.register('plugin.myMethod', {
 *     description: 'Do something',
 *     handler: async (params) => ({ result: 'done' }),
 *   });
 *
 *   const result = await methods.invoke('plugin.myMethod', { foo: 'bar' });
 */

// ============================================================================
// Types
// ============================================================================

/**
 * @typedef {Object} GatewayMethodDef
 * @property {string} method - Method name (e.g., 'plugin.myMethod')
 * @property {string} [description] - Human-readable description
 * @property {Function} handler - (params: Object, context: MethodContext) => Promise<Object>
 * @property {string} pluginId - Source plugin
 * @property {Object} [schema] - JSON Schema for params validation
 * @property {boolean} [requiresAuth=false] - Whether method requires authentication
 */

/**
 * @typedef {Object} MethodContext
 * @property {string} [senderId] - Caller identifier
 * @property {string} [channel] - Channel of origin
 * @property {Object} [session] - Session data
 */

/**
 * @typedef {Object} MethodResult
 * @property {boolean} ok
 * @property {Object} [result]
 * @property {string} [error]
 * @property {number} [durationMs]
 */

// ============================================================================
// GatewayMethodRegistry
// ============================================================================

export class GatewayMethodRegistry {
  constructor() {
    /** @type {Map<string, GatewayMethodDef>} */
    this._methods = new Map();
  }

  /**
   * Register a gateway method.
   *
   * @param {string} method - Method name (use dot notation: 'namespace.action')
   * @param {Object} opts
   * @param {string} [opts.description]
   * @param {Function} opts.handler
   * @param {string} opts.pluginId
   * @param {Object} [opts.schema]
   * @param {boolean} [opts.requiresAuth=false]
   */
  register(method, opts) {
    if (this._methods.has(method)) {
      throw new Error(`Gateway method "${method}" is already registered`);
    }

    if (!method || typeof method !== 'string') {
      throw new Error('Method name must be a non-empty string');
    }

    if (typeof opts.handler !== 'function') {
      throw new Error(`Handler for method "${method}" must be a function`);
    }

    if (!opts.pluginId) {
      throw new Error(`Plugin ID required for method "${method}"`);
    }

    this._methods.set(method, {
      method,
      description: opts.description || '',
      handler: opts.handler,
      pluginId: opts.pluginId,
      schema: opts.schema || null,
      requiresAuth: opts.requiresAuth === true,
    });
  }

  /**
   * Unregister a gateway method.
   *
   * @param {string} method
   * @returns {boolean}
   */
  unregister(method) {
    return this._methods.delete(method);
  }

  /**
   * Unregister all methods for a plugin.
   *
   * @param {string} pluginId
   * @returns {number} - Number of methods removed
   */
  unregisterPlugin(pluginId) {
    let count = 0;
    for (const [method, def] of this._methods) {
      if (def.pluginId === pluginId) {
        this._methods.delete(method);
        count++;
      }
    }
    return count;
  }

  /**
   * Check if a method is registered.
   *
   * @param {string} method
   * @returns {boolean}
   */
  has(method) {
    return this._methods.has(method);
  }

  /**
   * Get method definition.
   *
   * @param {string} method
   * @returns {GatewayMethodDef|null}
   */
  get(method) {
    return this._methods.get(method) || null;
  }

  /**
   * Invoke a gateway method.
   *
   * @param {string} method
   * @param {Object} [params={}]
   * @param {MethodContext} [context={}]
   * @returns {Promise<MethodResult>}
   */
  async invoke(method, params = {}, context = {}) {
    const def = this._methods.get(method);

    if (!def) {
      return { ok: false, error: `Unknown method: ${method}` };
    }

    // Basic schema validation
    if (def.schema) {
      const validationErrors = this._validateParams(params, def.schema);
      if (validationErrors.length > 0) {
        return { ok: false, error: `Validation failed: ${validationErrors.join(', ')}` };
      }
    }

    const startTime = Date.now();

    try {
      const result = await def.handler(params, context);
      return {
        ok: true,
        result,
        durationMs: Date.now() - startTime,
      };
    } catch (err) {
      return {
        ok: false,
        error: err.message,
        durationMs: Date.now() - startTime,
      };
    }
  }

  /**
   * List all registered methods.
   *
   * @param {Object} [opts]
   * @param {string} [opts.pluginId] - Filter by plugin
   * @param {string} [opts.prefix] - Filter by method name prefix
   * @returns {GatewayMethodDef[]}
   */
  list({ pluginId, prefix } = {}) {
    let methods = [...this._methods.values()];

    if (pluginId) {
      methods = methods.filter((m) => m.pluginId === pluginId);
    }

    if (prefix) {
      methods = methods.filter((m) => m.method.startsWith(prefix));
    }

    return methods;
  }

  /**
   * Get all method names grouped by namespace.
   *
   * @returns {Object<string, string[]>}
   */
  getNamespaces() {
    const namespaces = {};

    for (const method of this._methods.keys()) {
      const dotIdx = method.indexOf('.');
      const ns = dotIdx > 0 ? method.substring(0, dotIdx) : '_root';
      const name = dotIdx > 0 ? method.substring(dotIdx + 1) : method;

      if (!namespaces[ns]) {
        namespaces[ns] = [];
      }
      namespaces[ns].push(name);
    }

    return namespaces;
  }

  /**
   * Basic parameter validation.
   * @private
   */
  _validateParams(params, schema) {
    const errors = [];

    if (schema.required && Array.isArray(schema.required)) {
      for (const field of schema.required) {
        if (params[field] === undefined || params[field] === null) {
          errors.push(`Missing required parameter: "${field}"`);
        }
      }
    }

    if (schema.properties) {
      for (const [field, fieldSchema] of Object.entries(schema.properties)) {
        const value = params[field];
        if (value === undefined) continue;

        if (fieldSchema.type) {
          const actualType = Array.isArray(value) ? 'array' : typeof value;
          if (actualType !== fieldSchema.type) {
            errors.push(`Parameter "${field}": expected ${fieldSchema.type}, got ${actualType}`);
          }
        }
      }
    }

    return errors;
  }

  /**
   * Generate help text for all methods.
   *
   * @returns {string}
   */
  generateHelp() {
    if (this._methods.size === 0) return 'No gateway methods registered.';

    const namespaces = this.getNamespaces();
    const lines = ['Gateway Methods:', ''];

    for (const [ns, methods] of Object.entries(namespaces).sort()) {
      lines.push(`  ${ns}:`);
      for (const method of methods.sort()) {
        const fullName = ns === '_root' ? method : `${ns}.${method}`;
        const def = this._methods.get(fullName);
        const desc = def?.description ? ` - ${def.description}` : '';
        const auth = def?.requiresAuth ? ' [auth]' : '';
        lines.push(`    ${fullName}${desc}${auth}`);
      }
      lines.push('');
    }

    return lines.join('\n').trim();
  }

  /**
   * Clear all methods.
   */
  clear() {
    this._methods.clear();
  }
}

// ============================================================================
// Singleton
// ============================================================================

let _instance = null;

/**
 * Get the global GatewayMethodRegistry instance.
 *
 * @returns {GatewayMethodRegistry}
 */
export function getGatewayMethods() {
  if (!_instance) {
    _instance = new GatewayMethodRegistry();
  }
  return _instance;
}

/**
 * Reset the singleton (for testing).
 */
export function resetGatewayMethods() {
  if (_instance) {
    _instance.clear();
  }
  _instance = null;
}
