/**
 * browser-tools.js - Chrome DevTools Protocol (CDP) browser automation
 *
 * Provides headless Chrome control for StateSet iCommerce via the raw
 * Chrome DevTools Protocol over WebSocket.  No Puppeteer dependency --
 * only the `ws` npm package (or Node 22+ built-in WebSocket) is used.
 *
 * Usage:
 *   import { BrowserTools, getBrowserTools } from './browser/browser-tools.js';
 *   const browser = getBrowserTools({ headless: true });
 *   await browser.launch();
 *   await browser.navigate('https://example.com');
 *   const text = await browser.getPageContent();
 *   await browser.close();
 */

import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import http from 'node:http';
import { setTimeout as sleep } from 'node:timers/promises';

// ---------------------------------------------------------------------------
// WebSocket import -- prefer built-in (Node >= 22) then fall back to `ws`
// ---------------------------------------------------------------------------
let WebSocketImpl;
if (typeof globalThis.WebSocket !== 'undefined') {
  WebSocketImpl = globalThis.WebSocket;
} else {
  try {
    const ws = await import('ws');
    WebSocketImpl = ws.default || ws.WebSocket || ws;
  } catch (err) {
    console.debug(
      '[browser] WebSocket import failed (ws package not available):',
      err.message || err,
    );
    // Will throw a clear error at connect-time if neither is available.
    WebSocketImpl = null;
  }
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
const CHROME_PATHS = [
  '/usr/bin/chromium-browser',
  '/usr/bin/chromium',
  '/usr/bin/google-chrome',
  '/usr/bin/google-chrome-stable',
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
];

const DEFAULT_PORT = 9222;
const DEFAULT_TIMEOUT = 30_000;

// ---------------------------------------------------------------------------
// BrowserTools
// ---------------------------------------------------------------------------
export class BrowserTools {
  /**
   * @param {object} opts
   * @param {boolean}  [opts.headless=true]   Run Chrome in headless mode.
   * @param {string}   [opts.chromePath]      Explicit path to Chrome binary.
   * @param {number}   [opts.port=9222]       Remote-debugging port.
   * @param {number}   [opts.timeout=30000]   Default command timeout (ms).
   */
  constructor(opts = {}) {
    this.headless = opts.headless ?? true;
    this.chromePath = opts.chromePath ?? null;
    this.port = opts.port ?? DEFAULT_PORT;
    this.timeout = opts.timeout ?? DEFAULT_TIMEOUT;

    /** @type {import('child_process').ChildProcess | null} */
    this._process = null;
    /** @type {WebSocket | null} */
    this._ws = null;
    /** @type {Map<number, { resolve: Function, reject: Function }>} */
    this._pending = new Map();
    /** @type {number} */
    this._cmdId = 0;
    /** @type {boolean} */
    this._connected = false;
    /** @type {Map<string, Function[]>} */
    this._eventListeners = new Map();
  }

  // -----------------------------------------------------------------------
  // Chrome discovery
  // -----------------------------------------------------------------------

  /**
   * Resolve the Chrome / Chromium binary path.
   * @returns {string}
   */
  _findChrome() {
    if (this.chromePath) {
      if (!existsSync(this.chromePath)) {
        throw new Error(`Chrome binary not found at specified path: ${this.chromePath}`);
      }
      return this.chromePath;
    }

    for (const p of CHROME_PATHS) {
      if (existsSync(p)) {
        return p;
      }
    }

    throw new Error(
      'Could not find Chrome/Chromium. Searched:\n' +
        CHROME_PATHS.map((p) => `  - ${p}`).join('\n') +
        '\nProvide a path via the chromePath option.',
    );
  }

  // -----------------------------------------------------------------------
  // HTTP helper -- fetch /json/version from the debug port
  // -----------------------------------------------------------------------

  /**
   * GET a JSON endpoint on the Chrome debug port.
   * @param {string} path
   * @returns {Promise<any>}
   */
  _httpGet(path) {
    return new Promise((resolve, reject) => {
      const url = `http://127.0.0.1:${this.port}${path}`;
      http
        .get(url, (res) => {
          let data = '';
          res.on('data', (chunk) => {
            data += chunk;
          });
          res.on('end', () => {
            try {
              resolve(JSON.parse(data));
            } catch (err) {
              reject(new Error(`Failed to parse JSON from ${url}: ${err.message}`));
            }
          });
        })
        .on('error', reject);
    });
  }

  // -----------------------------------------------------------------------
  // WebSocket lifecycle
  // -----------------------------------------------------------------------

  /**
   * Wait for the Chrome debug port to become available, then retrieve
   * the WebSocket debugger URL and connect.
   * @returns {Promise<void>}
   */
  async _connect() {
    if (!WebSocketImpl) {
      throw new Error(
        'No WebSocket implementation available. Install the "ws" package ' +
          '(npm i ws) or use Node >= 22 which includes a built-in WebSocket.',
      );
    }

    // Poll until the debug port is ready.
    const deadline = Date.now() + this.timeout;
    let versionInfo;
    while (Date.now() < deadline) {
      try {
        versionInfo = await this._httpGet('/json/version');
        break;
      } catch (err) {
        console.debug('[browser] Waiting for Chrome debug port...', err.message || err);
        await sleep(200);
      }
    }

    if (!versionInfo) {
      throw new Error(
        `Timed out waiting for Chrome DevTools on port ${this.port} ` +
          `(waited ${this.timeout}ms).`,
      );
    }

    const wsUrl = versionInfo.webSocketDebuggerUrl;
    if (!wsUrl) {
      throw new Error('Chrome did not expose a webSocketDebuggerUrl.');
    }

    console.log(`[browser-tools] Connecting to Chrome CDP: ${wsUrl}`);

    await new Promise((resolve, reject) => {
      this._ws = new WebSocketImpl(wsUrl);

      const onOpen = () => {
        cleanup();
        this._connected = true;
        console.log('[browser-tools] CDP WebSocket connected.');
        resolve();
      };

      const onError = (err) => {
        cleanup();
        reject(new Error(`WebSocket connection error: ${err.message || err}`));
      };

      const cleanup = () => {
        this._ws.removeEventListener?.('open', onOpen);
        this._ws.removeEventListener?.('error', onError);
        // For `ws` module which uses Node EventEmitter API:
        this._ws.off?.('open', onOpen);
        this._ws.off?.('error', onError);
      };

      // Support both browser-style and Node `ws` style listeners.
      if (typeof this._ws.on === 'function') {
        this._ws.on('open', onOpen);
        this._ws.on('error', onError);
      } else {
        this._ws.addEventListener('open', onOpen);
        this._ws.addEventListener('error', onError);
      }
    });

    // Attach persistent message handler.
    const onMessage = (rawOrEvent) => {
      const raw = typeof rawOrEvent === 'string' ? rawOrEvent : (rawOrEvent.data ?? rawOrEvent);
      let msg;
      try {
        msg = JSON.parse(typeof raw === 'string' ? raw : raw.toString());
      } catch (err) {
        console.debug('[browser] Failed to parse CDP message:', err.message || err);
        return;
      }

      // Response to a command we sent.
      if (msg.id !== undefined && this._pending.has(msg.id)) {
        const { resolve: res, reject: rej } = this._pending.get(msg.id);
        this._pending.delete(msg.id);
        if (msg.error) {
          rej(new Error(`CDP error (${msg.error.code}): ${msg.error.message}`));
        } else {
          res(msg.result);
        }
        return;
      }

      // Event.
      if (msg.method) {
        const listeners = this._eventListeners.get(msg.method);
        if (listeners) {
          for (const fn of listeners) {
            try {
              fn(msg.params);
            } catch (err) {
              console.debug('[browser] CDP event listener error:', err.message || err);
            }
          }
        }
      }
    };

    if (typeof this._ws.on === 'function') {
      this._ws.on('message', onMessage);
    } else {
      this._ws.addEventListener('message', onMessage);
    }
  }

  // -----------------------------------------------------------------------
  // CDP command helpers
  // -----------------------------------------------------------------------

  /**
   * Send a raw CDP command and wait for the response.
   * @param {string} method  CDP method name (e.g. "Page.navigate").
   * @param {object} [params={}]
   * @returns {Promise<any>}
   */
  send(method, params = {}) {
    if (!this._ws || !this._connected) {
      throw new Error('Not connected to Chrome. Call launch() first.');
    }

    const id = ++this._cmdId;
    const payload = JSON.stringify({ id, method, params });

    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this._pending.delete(id);
        reject(new Error(`CDP command "${method}" timed out after ${this.timeout}ms`));
      }, this.timeout);

      this._pending.set(id, {
        resolve: (result) => {
          clearTimeout(timer);
          resolve(result);
        },
        reject: (err) => {
          clearTimeout(timer);
          reject(err);
        },
      });

      this._ws.send(payload);
    });
  }

  /**
   * Register a listener for a CDP event.
   * @param {string} event
   * @param {Function} fn
   */
  on(event, fn) {
    if (!this._eventListeners.has(event)) {
      this._eventListeners.set(event, []);
    }
    this._eventListeners.get(event).push(fn);
  }

  /**
   * Remove a listener for a CDP event.
   * @param {string} event
   * @param {Function} fn
   */
  off(event, fn) {
    const arr = this._eventListeners.get(event);
    if (arr) {
      const idx = arr.indexOf(fn);
      if (idx !== -1) arr.splice(idx, 1);
    }
  }

  // -----------------------------------------------------------------------
  // Public API
  // -----------------------------------------------------------------------

  /**
   * Launch Chrome (if needed) and connect via CDP.
   * @returns {Promise<void>}
   */
  async launch() {
    // Check if there is already a Chrome listening on the port.
    let alreadyRunning = false;
    try {
      await this._httpGet('/json/version');
      alreadyRunning = true;
      console.log(`[browser-tools] Found existing Chrome on port ${this.port}.`);
    } catch (err) {
      console.debug('[browser] No existing Chrome found, will launch:', err.message || err);
      // Not running -- we will launch one.
    }

    if (!alreadyRunning) {
      const chromeBin = this._findChrome();
      console.log(`[browser-tools] Launching Chrome: ${chromeBin}`);

      const args = [
        `--remote-debugging-port=${this.port}`,
        '--no-first-run',
        '--no-default-browser-check',
        '--disable-background-networking',
        '--disable-background-timer-throttling',
        '--disable-client-side-phishing-detection',
        '--disable-default-apps',
        '--disable-extensions',
        '--disable-hang-monitor',
        '--disable-popup-blocking',
        '--disable-prompt-on-repost',
        '--disable-sync',
        '--disable-translate',
        '--metrics-recording-only',
        '--safebrowsing-disable-auto-update',
        '--no-sandbox',
        '--disable-gpu',
        '--disable-dev-shm-usage',
      ];

      if (this.headless) {
        args.push('--headless=new');
      }

      // Open about:blank so Chrome has a usable page target.
      args.push('about:blank');

      this._process = spawn(chromeBin, args, {
        stdio: ['ignore', 'pipe', 'pipe'],
        detached: false,
      });

      this._process.on('error', (err) => {
        console.error(`[browser-tools] Chrome process error: ${err.message}`);
      });

      this._process.on('exit', (code, signal) => {
        console.log(`[browser-tools] Chrome exited (code=${code}, signal=${signal}).`);
        this._process = null;
      });
    }

    // Connect over CDP.
    await this._connect();

    // Enable domains we rely on.
    await this.send('Page.enable');
    await this.send('Runtime.enable');
    await this.send('DOM.enable');

    // If Chrome was freshly launched, navigate the default about:blank target
    // to ensure we have a proper page context.
    if (!alreadyRunning) {
      // The first target is typically about:blank; nothing extra needed.
    }
  }

  /**
   * Navigate to a URL and wait for the page to load.
   * @param {string} url
   * @returns {Promise<void>}
   */
  async navigate(url) {
    // Set up a promise that resolves when the load event fires.
    const loaded = new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.off('Page.loadEventFired', handler);
        reject(new Error(`Navigation to ${url} timed out after ${this.timeout}ms`));
      }, this.timeout);

      const handler = () => {
        clearTimeout(timer);
        this.off('Page.loadEventFired', handler);
        resolve();
      };

      this.on('Page.loadEventFired', handler);
    });

    const result = await this.send('Page.navigate', { url });

    if (result.errorText) {
      throw new Error(`Navigation failed: ${result.errorText}`);
    }

    await loaded;
  }

  /**
   * Get the visible text content of the page (document.body.innerText).
   * @returns {Promise<string>}
   */
  async getPageContent() {
    const { result } = await this.send('Runtime.evaluate', {
      expression: 'document.body.innerText',
      returnByValue: true,
    });

    if (result.subtype === 'error') {
      throw new Error(`getPageContent failed: ${result.description}`);
    }

    return result.value ?? '';
  }

  /**
   * Get the full HTML of the page (document.documentElement.outerHTML).
   * @returns {Promise<string>}
   */
  async getPageHTML() {
    const { result } = await this.send('Runtime.evaluate', {
      expression: 'document.documentElement.outerHTML',
      returnByValue: true,
    });

    if (result.subtype === 'error') {
      throw new Error(`getPageHTML failed: ${result.description}`);
    }

    return result.value ?? '';
  }

  /**
   * Take a screenshot of the current page.
   * @param {object} [opts]
   * @param {'png'|'jpeg'|'webp'} [opts.format='png']
   * @param {number} [opts.quality]  JPEG/WebP quality (0-100).
   * @param {boolean} [opts.fullPage=false] Capture beyond the viewport.
   * @returns {Promise<Buffer>}
   */
  async screenshot(opts = {}) {
    const format = opts.format ?? 'png';
    const params = { format };

    if (opts.quality !== undefined && format !== 'png') {
      params.quality = opts.quality;
    }

    if (opts.fullPage) {
      // Get full page dimensions.
      const metrics = await this.send('Page.getLayoutMetrics');
      const { width, height } = metrics.cssContentSize || metrics.contentSize;
      params.clip = { x: 0, y: 0, width, height, scale: 1 };
    }

    const { data } = await this.send('Page.captureScreenshot', params);
    return Buffer.from(data, 'base64');
  }

  /**
   * Evaluate a JavaScript expression in the page context.
   * @param {string} expression
   * @returns {Promise<any>}
   */
  async evaluate(expression) {
    const { result, exceptionDetails } = await this.send('Runtime.evaluate', {
      expression,
      returnByValue: true,
      awaitPromise: true,
    });

    if (exceptionDetails) {
      const text =
        exceptionDetails.exception?.description ||
        exceptionDetails.text ||
        'Unknown evaluation error';
      throw new Error(`evaluate() failed: ${text}`);
    }

    return result.value;
  }

  /**
   * Click an element identified by a CSS selector.
   * @param {string} selector
   * @returns {Promise<void>}
   */
  async click(selector) {
    // Scroll the element into view and retrieve its center coordinates.
    const coords = await this.evaluate(`
      (() => {
        const el = document.querySelector(${JSON.stringify(selector)});
        if (!el) throw new Error('Element not found: ${selector.replace(/'/g, "\\'")}');
        el.scrollIntoView({ block: 'center', inline: 'center' });
        const rect = el.getBoundingClientRect();
        return { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 };
      })()
    `);

    // Simulate mouse events via Input domain.
    await this.send('Input.dispatchMouseEvent', {
      type: 'mousePressed',
      x: coords.x,
      y: coords.y,
      button: 'left',
      clickCount: 1,
    });

    await this.send('Input.dispatchMouseEvent', {
      type: 'mouseReleased',
      x: coords.x,
      y: coords.y,
      button: 'left',
      clickCount: 1,
    });
  }

  /**
   * Type text into an element identified by a CSS selector.
   * Focuses the element first, then dispatches key events.
   * @param {string} selector
   * @param {string} text
   * @returns {Promise<void>}
   */
  async type(selector, text) {
    // Focus the element.
    await this.evaluate(`
      (() => {
        const el = document.querySelector(${JSON.stringify(selector)});
        if (!el) throw new Error('Element not found: ${selector.replace(/'/g, "\\'")}');
        el.focus();
      })()
    `);

    // Type each character via Input.dispatchKeyEvent.
    for (const char of text) {
      await this.send('Input.dispatchKeyEvent', {
        type: 'keyDown',
        text: char,
        key: char,
        unmodifiedText: char,
      });
      await this.send('Input.dispatchKeyEvent', {
        type: 'keyUp',
        key: char,
      });
    }
  }

  /**
   * Wait for an element matching the selector to appear in the DOM.
   * @param {string} selector
   * @param {number} [timeout]  Override default timeout.
   * @returns {Promise<void>}
   */
  async waitForSelector(selector, timeout) {
    const deadline = Date.now() + (timeout ?? this.timeout);
    const escapedSelector = JSON.stringify(selector);

    while (Date.now() < deadline) {
      const found = await this.evaluate(`!!document.querySelector(${escapedSelector})`);
      if (found) return;
      await sleep(100);
    }

    throw new Error(`waitForSelector("${selector}") timed out after ${timeout ?? this.timeout}ms`);
  }

  /**
   * Extract all links (<a href>) from the current page.
   * @returns {Promise<Array<{ href: string, text: string }>>}
   */
  async extractLinks() {
    return this.evaluate(`
      Array.from(document.querySelectorAll('a[href]')).map(a => ({
        href: a.href,
        text: (a.innerText || '').trim(),
      }))
    `);
  }

  /**
   * Extract the text content of a specific element.
   * @param {string} selector
   * @returns {Promise<string>}
   */
  async extractText(selector) {
    const escapedSelector = JSON.stringify(selector);
    const text = await this.evaluate(`
      (() => {
        const el = document.querySelector(${escapedSelector});
        if (!el) return null;
        return el.innerText;
      })()
    `);

    if (text === null) {
      throw new Error(`extractText: element not found for selector "${selector}"`);
    }

    return text;
  }

  // -----------------------------------------------------------------------
  // Semantic Snapshots (Accessibility Tree)
  // -----------------------------------------------------------------------

  /**
   * Get the accessibility tree for the current page.
   * Returns a lightweight, token-efficient representation of the page structure
   * suitable for LLM consumption (~100x smaller than screenshots).
   *
   * @param {object} [opts]
   * @param {number} [opts.depth=5] - Maximum tree depth to traverse
   * @param {boolean} [opts.interestingOnly=true] - Only include interactive/semantic elements
   * @returns {Promise<object>} - The accessibility tree root node
   */
  async getAccessibilityTree(opts = {}) {
    const depth = opts.depth ?? 5;
    const interestingOnly = opts.interestingOnly ?? true;

    // Enable Accessibility domain if not already enabled
    await this.send('Accessibility.enable');

    // Get the full accessibility tree
    const { nodes } = await this.send('Accessibility.getFullAXTree', {
      depth,
      max_depth: depth,
    });

    if (!nodes || nodes.length === 0) {
      return { role: 'RootWebArea', name: '', children: [] };
    }

    // Build a map of nodeId -> node for quick lookup
    const nodeMap = new Map();
    for (const node of nodes) {
      nodeMap.set(node.nodeId, node);
    }

    // Convert CDP accessibility nodes to our simplified format
    const convertNode = (cdpNode, currentDepth = 0) => {
      if (!cdpNode || currentDepth > depth) return null;

      const role = cdpNode.role?.value || 'unknown';
      const name = cdpNode.name?.value || '';
      const value = cdpNode.value?.value || '';
      const description = cdpNode.description?.value || '';

      // Filter out uninteresting nodes if requested
      if (interestingOnly) {
        const interestingRoles = new Set([
          'button',
          'link',
          'textbox',
          'checkbox',
          'radio',
          'combobox',
          'listbox',
          'option',
          'menuitem',
          'tab',
          'tabpanel',
          'dialog',
          'alert',
          'alertdialog',
          'heading',
          'img',
          'list',
          'listitem',
          'table',
          'row',
          'cell',
          'form',
          'search',
          'navigation',
          'main',
          'article',
          'banner',
          'contentinfo',
          'complementary',
          'region',
          'slider',
          'spinbutton',
          'switch',
          'textfield',
          'searchbox',
        ]);

        // Skip generic/container nodes without meaningful content
        if (!interestingRoles.has(role.toLowerCase()) && !name && !value) {
          // But still process children
          const children = [];
          if (cdpNode.childIds) {
            for (const childId of cdpNode.childIds) {
              const childNode = nodeMap.get(childId);
              if (childNode) {
                const converted = convertNode(childNode, currentDepth);
                if (converted) {
                  if (Array.isArray(converted)) {
                    children.push(...converted);
                  } else {
                    children.push(converted);
                  }
                }
              }
            }
          }
          return children.length > 0 ? children : null;
        }
      }

      const result = {
        role: role.toLowerCase(),
        ...(name && { name }),
        ...(value && { value }),
        ...(description && { description }),
      };

      // Add properties that indicate state
      if (cdpNode.properties) {
        const props = {};
        for (const prop of cdpNode.properties) {
          const propName = prop.name;
          const propValue = prop.value?.value;
          if (propValue !== undefined && propValue !== false) {
            // Include boolean states and values
            if (
              [
                'disabled',
                'checked',
                'selected',
                'expanded',
                'pressed',
                'required',
                'readonly',
              ].includes(propName)
            ) {
              props[propName] = propValue;
            }
          }
        }
        if (Object.keys(props).length > 0) {
          result.properties = props;
        }
      }

      // Process children
      if (cdpNode.childIds && cdpNode.childIds.length > 0) {
        const children = [];
        for (const childId of cdpNode.childIds) {
          const childNode = nodeMap.get(childId);
          if (childNode) {
            const converted = convertNode(childNode, currentDepth + 1);
            if (converted) {
              if (Array.isArray(converted)) {
                children.push(...converted);
              } else {
                children.push(converted);
              }
            }
          }
        }
        if (children.length > 0) {
          result.children = children;
        }
      }

      return result;
    };

    // Find the root node (usually the first one with role RootWebArea)
    const rootNode = nodes.find((n) => n.role?.value === 'RootWebArea') || nodes[0];
    return convertNode(rootNode) || { role: 'RootWebArea', name: '', children: [] };
  }

  /**
   * Get a semantic snapshot of the page in a text format optimized for LLMs.
   * This is the recommended method for agent-based browsing.
   *
   * Returns a compact text representation like:
   *   - button "Sign In" [ref=1]
   *   - textbox "Email" [ref=2]
   *   - heading "Welcome back"
   *
   * @param {object} [opts]
   * @param {number} [opts.depth=5] - Maximum tree depth
   * @param {boolean} [opts.includeRefs=true] - Include reference IDs for interaction
   * @returns {Promise<{ snapshot: string, refs: Map<number, object> }>}
   */
  async getSemanticSnapshot(opts = {}) {
    const depth = opts.depth ?? 5;
    const includeRefs = opts.includeRefs ?? true;

    const tree = await this.getAccessibilityTree({ depth, interestingOnly: true });

    let refCounter = 0;
    const refs = new Map();
    const lines = [];

    const formatNode = (node, indent = 0) => {
      if (!node || typeof node !== 'object') return;

      const prefix = '  '.repeat(indent) + '- ';
      let line = prefix + node.role;

      // Add name/value in quotes
      if (node.name) {
        line += ` "${node.name}"`;
      } else if (node.value) {
        line += ` "${node.value}"`;
      }

      // Add state indicators
      if (node.properties) {
        const states = [];
        if (node.properties.disabled) states.push('disabled');
        if (node.properties.checked) states.push('checked');
        if (node.properties.selected) states.push('selected');
        if (node.properties.expanded) states.push('expanded');
        if (node.properties.required) states.push('required');
        if (states.length > 0) {
          line += ` (${states.join(', ')})`;
        }
      }

      // Add reference ID for interactive elements
      if (includeRefs) {
        const interactiveRoles = new Set([
          'button',
          'link',
          'textbox',
          'checkbox',
          'radio',
          'combobox',
          'listbox',
          'option',
          'menuitem',
          'tab',
          'slider',
          'switch',
          'spinbutton',
          'searchbox',
          'textfield',
        ]);

        if (interactiveRoles.has(node.role)) {
          refCounter++;
          refs.set(refCounter, {
            role: node.role,
            name: node.name || node.value || '',
            nodeInfo: node,
          });
          line += ` [ref=${refCounter}]`;
        }
      }

      lines.push(line);

      // Process children
      if (node.children) {
        for (const child of node.children) {
          formatNode(child, indent + 1);
        }
      }
    };

    // Handle case where tree is array (from filtering)
    if (Array.isArray(tree)) {
      for (const node of tree) {
        formatNode(node, 0);
      }
    } else {
      // Start from children of root (skip the RootWebArea wrapper)
      if (tree.children) {
        for (const child of tree.children) {
          formatNode(child, 0);
        }
      } else {
        formatNode(tree, 0);
      }
    }

    return {
      snapshot: lines.join('\n'),
      refs,
      nodeCount: lines.length,
      refCount: refCounter,
    };
  }

  /**
   * Interact with an element by reference ID from a semantic snapshot.
   *
   * @param {number} refId - The reference ID from getSemanticSnapshot()
   * @param {string} action - 'click', 'type', or 'focus'
   * @param {string} [value] - Text to type (for 'type' action)
   * @param {Map<number, object>} refs - The refs map from getSemanticSnapshot()
   * @returns {Promise<void>}
   */
  async interactByRef(refId, action, value, refs) {
    const ref = refs.get(refId);
    if (!ref) {
      throw new Error(`Reference ${refId} not found in snapshot. Re-fetch the snapshot.`);
    }

    // Use the accessibility node to find the DOM element
    // We'll search by role and accessible name
    const { role, name } = ref;

    // Build a selector strategy based on role and name
    const selector = await this.evaluate(`
      (() => {
        // Try to find by aria-label or text content
        const elements = document.querySelectorAll('[role="${role}"], ${role}');
        for (const el of elements) {
          const ariaLabel = el.getAttribute('aria-label') || '';
          const text = (el.innerText || el.value || '').trim();
          if (ariaLabel === "${name.replace(/"/g, '\\"')}" ||
              text === "${name.replace(/"/g, '\\"')}" ||
              el.placeholder === "${name.replace(/"/g, '\\"')}") {
            // Generate a unique selector
            if (el.id) return '#' + el.id;
            if (el.name) return '[name="' + el.name + '"]';
            // Use nth-of-type as fallback
            const parent = el.parentElement;
            if (parent) {
              const siblings = Array.from(parent.children).filter(c => c.tagName === el.tagName);
              const idx = siblings.indexOf(el) + 1;
              return el.tagName.toLowerCase() + ':nth-of-type(' + idx + ')';
            }
            return el.tagName.toLowerCase();
          }
        }
        return null;
      })()
    `);

    if (!selector) {
      throw new Error(`Could not find DOM element for ref ${refId} (${role}: "${name}")`);
    }

    switch (action) {
      case 'click':
        await this.click(selector);
        break;
      case 'type':
        if (!value) throw new Error('Value required for type action');
        await this.type(selector, value);
        break;
      case 'focus':
        await this.evaluate(`document.querySelector(${JSON.stringify(selector)}).focus()`);
        break;
      default:
        throw new Error(`Unknown action: ${action}`);
    }
  }

  /**
   * Close the browser and clean up resources.
   * @returns {Promise<void>}
   */
  async close() {
    // Try graceful CDP close first.
    if (this._ws && this._connected) {
      try {
        await this.send('Browser.close');
      } catch (err) {
        console.debug(
          '[browser] Browser.close CDP command failed (browser may already be gone):',
          err.message || err,
        );
      }
    }

    // Tear down the WebSocket.
    if (this._ws) {
      try {
        this._ws.close();
      } catch (err) {
        console.debug('[browser] WebSocket close failed:', err.message || err);
      }
      this._ws = null;
    }

    this._connected = false;
    this._pending.clear();
    this._eventListeners.clear();

    // Kill the child process if we spawned it.
    if (this._process) {
      try {
        this._process.kill('SIGTERM');
      } catch (err) {
        console.debug('[browser] Failed to kill Chrome process:', err.message || err);
      }
      this._process = null;
    }

    console.log('[browser-tools] Browser closed.');
  }
}

// ---------------------------------------------------------------------------
// Singleton
// ---------------------------------------------------------------------------

/** @type {BrowserTools | null} */
let _instance = null;

/**
 * Return a singleton BrowserTools instance.  Options are only applied on the
 * first call; subsequent calls return the existing instance.
 *
 * @param {object} [opts]  Same options as the BrowserTools constructor.
 * @returns {BrowserTools}
 */
export function getBrowserTools(opts = {}) {
  if (!_instance) {
    _instance = new BrowserTools(opts);
  }
  return _instance;
}
