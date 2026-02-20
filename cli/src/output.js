/**
 * Rich Output Formatting for StateSet CLI
 *
 * Provides tables, progress indicators, formatted displays,
 * and consistent styling for CLI output.
 */

// ============================================================================
// ANSI Color Codes (for terminal styling)
// ============================================================================

const COLORS = {
  reset: '\x1b[0m',
  bold: '\x1b[1m',
  dim: '\x1b[2m',
  italic: '\x1b[3m',
  underline: '\x1b[4m',

  // Foreground colors
  black: '\x1b[30m',
  red: '\x1b[31m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  magenta: '\x1b[35m',
  cyan: '\x1b[36m',
  white: '\x1b[37m',
  gray: '\x1b[90m',

  // Background colors
  bgRed: '\x1b[41m',
  bgGreen: '\x1b[42m',
  bgYellow: '\x1b[43m',
  bgBlue: '\x1b[44m',
};

// ============================================================================
// Status Icons
// ============================================================================

export const ICONS = {
  success: '✅',
  error: '❌',
  warning: '⚠️',
  info: 'ℹ️',
  loading: '⏳',
  tool: '🔧',
  order: '📦',
  cart: '🛒',
  customer: '👤',
  product: '📦',
  inventory: '📊',
  return: '↩️',
  payment: '💳',
  ship: '🚚',
  analytics: '📈',
  currency: '💱',
  check: '✓',
  cross: '✗',
  arrow: '→',
  bullet: '•',
  database: '🗄️',
  session: '💾',
};

// ============================================================================
// Main Output Class
// ============================================================================

/**
 * RichOutput - Formatted output utilities for CLI
 *
 * Usage:
 *   const output = new RichOutput({ color: true });
 *   console.log(output.table(data, columns));
 *   console.log(output.orderCard(order));
 */
export class RichOutput {
  constructor(options = {}) {
    this.color = options.color !== false && process.stdout.isTTY;
    this.format = options.format || 'pretty'; // 'pretty' | 'json' | 'minimal'
    this.width = options.width || process.stdout.columns || 80;
  }

  // --------------------------------------------------------------------------
  // Color Helpers
  // --------------------------------------------------------------------------

  _c(color, text) {
    if (!this.color) return text;
    return `${COLORS[color] || ''}${text}${COLORS.reset}`;
  }

  bold(text) {
    return this._c('bold', text);
  }
  dim(text) {
    return this._c('dim', text);
  }
  green(text) {
    return this._c('green', text);
  }
  red(text) {
    return this._c('red', text);
  }
  yellow(text) {
    return this._c('yellow', text);
  }
  blue(text) {
    return this._c('blue', text);
  }
  cyan(text) {
    return this._c('cyan', text);
  }
  gray(text) {
    return this._c('gray', text);
  }

  // --------------------------------------------------------------------------
  // Table Formatting
  // --------------------------------------------------------------------------

  /**
   * Format data as a table
   *
   * @param {Array} data - Array of objects
   * @param {Array} columns - Column definitions [{ key, header, width?, align?, format? }]
   */
  table(data, columns) {
    if (this.format === 'json') {
      return JSON.stringify(data, null, 2);
    }

    if (!data || data.length === 0) {
      return this.dim('  (no data)');
    }

    // Calculate column widths
    const widths = {};
    for (const col of columns) {
      const headerLen = col.header.length;
      const maxDataLen = Math.max(
        ...data.map((row) => {
          let val = row[col.key];
          if (col.format) val = col.format(val, row);
          return String(val ?? '').length;
        }),
      );
      widths[col.key] = col.width || Math.min(Math.max(headerLen, maxDataLen), 40);
    }

    // Build header
    const headerCells = columns.map((col) =>
      this._padCell(this.bold(col.header), widths[col.key], col.align || 'left'),
    );
    const header = '  ' + headerCells.join(' │ ');

    // Separator
    const separator = '  ' + columns.map((col) => '─'.repeat(widths[col.key])).join('─┼─');

    // Build rows
    const rows = data.map((row) => {
      const cells = columns.map((col) => {
        let val = row[col.key];
        if (col.format) val = col.format(val, row);
        val = String(val ?? '');
        return this._padCell(val, widths[col.key], col.align || 'left');
      });
      return '  ' + cells.join(' │ ');
    });

    return [header, separator, ...rows].join('\n');
  }

  _padCell(text, width, align) {
    // Strip ANSI codes for length calculation
    // eslint-disable-next-line no-control-regex
    const visibleLength = text.replace(/\x1b\[[0-9;]*m/g, '').length;
    const padding = Math.max(0, width - visibleLength);

    if (align === 'right') {
      return ' '.repeat(padding) + text;
    } else if (align === 'center') {
      const left = Math.floor(padding / 2);
      const right = padding - left;
      return ' '.repeat(left) + text + ' '.repeat(right);
    }
    return text + ' '.repeat(padding);
  }

  // --------------------------------------------------------------------------
  // Progress & Status
  // --------------------------------------------------------------------------

  /**
   * Progress bar
   */
  progress(current, total, label = '') {
    const pct = total > 0 ? Math.round((current / total) * 100) : 0;
    const filled = Math.round(pct / 5);
    const bar = this.green('█'.repeat(filled)) + this.dim('░'.repeat(20 - filled));
    return `${label} [${bar}] ${pct}% (${current}/${total})`;
  }

  /**
   * Status message with icon
   */
  status(type, message) {
    const icon = ICONS[type] || '';
    const colorFn = {
      success: 'green',
      error: 'red',
      warning: 'yellow',
      info: 'cyan',
    }[type];

    const text = colorFn ? this[colorFn](message) : message;
    return `${icon} ${text}`;
  }

  /**
   * Spinner frames for loading animation
   */
  static SPINNER_FRAMES = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

  spinner(message, frame = 0) {
    const spin = RichOutput.SPINNER_FRAMES[frame % RichOutput.SPINNER_FRAMES.length];
    return `${this.cyan(spin)} ${message}`;
  }

  // --------------------------------------------------------------------------
  // Currency & Numbers
  // --------------------------------------------------------------------------

  /**
   * Format currency with symbol
   */
  currency(amount, code = 'USD') {
    if (amount === null || amount === undefined) return '—';

    try {
      return new Intl.NumberFormat('en-US', {
        style: 'currency',
        currency: code,
      }).format(amount);
    } catch (err) {
      console.debug('[output] Currency formatting failed for', code, ':', err.message || err);
      return `${code} ${Number(amount).toFixed(2)}`;
    }
  }

  /**
   * Format number with commas
   */
  number(value, decimals = 0) {
    if (value === null || value === undefined) return '—';
    return Number(value).toLocaleString('en-US', {
      minimumFractionDigits: decimals,
      maximumFractionDigits: decimals,
    });
  }

  /**
   * Format percentage
   */
  percent(value, decimals = 1) {
    if (value === null || value === undefined) return '—';
    return `${Number(value).toFixed(decimals)}%`;
  }

  // --------------------------------------------------------------------------
  // Date & Time
  // --------------------------------------------------------------------------

  /**
   * Format date
   */
  date(value) {
    if (!value) return '—';
    const d = new Date(value);
    return d.toLocaleDateString('en-US', {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    });
  }

  /**
   * Format datetime
   */
  datetime(value) {
    if (!value) return '—';
    const d = new Date(value);
    return d.toLocaleString('en-US', {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  }

  /**
   * Relative time (e.g., "2 hours ago")
   */
  relativeTime(value) {
    if (!value) return '—';
    const d = new Date(value);
    const now = new Date();
    const diffMs = now - d;
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);

    if (diffMins < 1) return 'just now';
    if (diffMins < 60) return `${diffMins}m ago`;
    if (diffHours < 24) return `${diffHours}h ago`;
    if (diffDays < 7) return `${diffDays}d ago`;
    return this.date(value);
  }

  // --------------------------------------------------------------------------
  // Status Badges
  // --------------------------------------------------------------------------

  /**
   * Format order status with color
   */
  orderStatus(status) {
    const styles = {
      pending: { color: 'yellow', icon: '○' },
      confirmed: { color: 'blue', icon: '◐' },
      processing: { color: 'cyan', icon: '◑' },
      shipped: { color: 'magenta', icon: '◕' },
      delivered: { color: 'green', icon: '●' },
      cancelled: { color: 'red', icon: '✗' },
      refunded: { color: 'gray', icon: '↩' },
    };

    const style = styles[status] || { color: 'gray', icon: '?' };
    return this._c(style.color, `${style.icon} ${status}`);
  }

  /**
   * Format payment status
   */
  paymentStatus(status) {
    const styles = {
      pending: 'yellow',
      processing: 'cyan',
      completed: 'green',
      failed: 'red',
      refunded: 'gray',
    };
    return this._c(styles[status] || 'gray', status);
  }

  /**
   * Format inventory status
   */
  inventoryStatus(available, reorderPoint = 0) {
    if (available <= 0) {
      return this.red('Out of Stock');
    } else if (available <= reorderPoint) {
      return this.yellow(`Low Stock (${available})`);
    }
    return this.green(`In Stock (${available})`);
  }

  // --------------------------------------------------------------------------
  // Cards & Boxes
  // --------------------------------------------------------------------------

  /**
   * Format an order as a card
   */
  orderCard(order) {
    const width = 45;
    const hr = '─'.repeat(width - 2);

    const lines = [
      `┌${hr}┐`,
      `│ ${this.bold('Order: ' + (order.orderNumber || order.id).slice(0, width - 12)).padEnd(width - 3)}│`,
      `├${hr}┤`,
      `│ Status:   ${this.orderStatus(order.status).padEnd(width - 14 + 10)}│`,
      `│ Customer: ${(order.customerEmail || order.customerId || '—').slice(0, width - 15).padEnd(width - 14)}│`,
      `│ Total:    ${this.currency(order.totalAmount, order.currency).padEnd(width - 14)}│`,
      `│ Items:    ${String(order.items?.length || order.itemCount || 0).padEnd(width - 14)}│`,
      `│ Created:  ${this.relativeTime(order.createdAt).padEnd(width - 14)}│`,
      `└${hr}┘`,
    ];

    return lines.join('\n');
  }

  /**
   * Format a cart as a card
   */
  cartCard(cart) {
    const width = 45;
    const hr = '─'.repeat(width - 2);

    const lines = [
      `┌${hr}┐`,
      `│ ${this.bold('Cart: ' + (cart.cartNumber || cart.id).slice(0, width - 10)).padEnd(width - 3)}│`,
      `├${hr}┤`,
      `│ Status:   ${(cart.status || '—').padEnd(width - 14)}│`,
      `│ Customer: ${(cart.customerEmail || 'Guest').slice(0, width - 15).padEnd(width - 14)}│`,
      `│ Items:    ${String(cart.itemCount || cart.items?.length || 0).padEnd(width - 14)}│`,
      `│ Subtotal: ${this.currency(cart.subtotal, cart.currency).padEnd(width - 14)}│`,
      `│ Total:    ${this.bold(this.currency(cart.grandTotal, cart.currency)).padEnd(width - 14 + 8)}│`,
      `└${hr}┘`,
    ];

    return lines.join('\n');
  }

  /**
   * Format a customer as a card
   */
  customerCard(customer) {
    const width = 45;
    const hr = '─'.repeat(width - 2);

    const name = `${customer.firstName || ''} ${customer.lastName || ''}`.trim() || '—';

    const lines = [
      `┌${hr}┐`,
      `│ ${ICONS.customer} ${this.bold(name.slice(0, width - 8)).padEnd(width - 5)}│`,
      `├${hr}┤`,
      `│ Email:  ${(customer.email || '—').slice(0, width - 12).padEnd(width - 11)}│`,
      `│ Phone:  ${(customer.phone || '—').slice(0, width - 12).padEnd(width - 11)}│`,
      `│ Status: ${(customer.status || '—').padEnd(width - 11)}│`,
      `│ Since:  ${this.date(customer.createdAt).padEnd(width - 11)}│`,
      `└${hr}┘`,
    ];

    return lines.join('\n');
  }

  // --------------------------------------------------------------------------
  // Lists & Key-Value
  // --------------------------------------------------------------------------

  /**
   * Format key-value pairs
   */
  keyValue(data, options = {}) {
    const labelWidth = options.labelWidth || 15;
    const lines = [];

    for (const [key, value] of Object.entries(data)) {
      const label = this.dim((key + ':').padEnd(labelWidth));
      lines.push(`  ${label} ${value}`);
    }

    return lines.join('\n');
  }

  /**
   * Bullet list
   */
  list(items, options = {}) {
    const bullet = options.bullet || ICONS.bullet;
    return items.map((item) => `  ${bullet} ${item}`).join('\n');
  }

  /**
   * Numbered list
   */
  numberedList(items) {
    return items.map((item, i) => `  ${i + 1}. ${item}`).join('\n');
  }

  // --------------------------------------------------------------------------
  // Headers & Dividers
  // --------------------------------------------------------------------------

  /**
   * Section header
   */
  header(text, icon = '') {
    const prefix = icon ? `${icon} ` : '';
    return `\n${prefix}${this.bold(text)}\n${'─'.repeat(text.length + (icon ? 2 : 0))}`;
  }

  /**
   * Horizontal rule
   */
  hr(char = '─') {
    return char.repeat(Math.min(this.width, 50));
  }

  /**
   * Box around text
   */
  box(text, options = {}) {
    const lines = text.split('\n');
    const maxLen = Math.max(...lines.map((l) => l.length));
    const width = Math.min(maxLen + 4, this.width);
    const title = options.title || '';

    const top = title
      ? `┌─ ${title} ${'─'.repeat(width - title.length - 5)}┐`
      : `┌${'─'.repeat(width - 2)}┐`;

    const middle = lines.map((line) => `│ ${line.padEnd(width - 4)} │`);

    const bottom = `└${'─'.repeat(width - 2)}┘`;

    return [top, ...middle, bottom].join('\n');
  }

  // --------------------------------------------------------------------------
  // Tool Call Formatting
  // --------------------------------------------------------------------------

  /**
   * Format a tool call for display
   */
  toolCall(name, input, options = {}) {
    const shortName = name.replace('mcp__stateset-commerce__', '');
    const inputStr = JSON.stringify(input);
    const truncatedInput = inputStr.length > 60 ? inputStr.slice(0, 57) + '...' : inputStr;

    if (options.showDuration) {
      return `${ICONS.tool} ${this.cyan(shortName)}(${this.dim(truncatedInput)}) ${this.dim(`[${options.duration}ms]`)}`;
    }

    return `${ICONS.tool} ${this.cyan(shortName)}(${this.dim(truncatedInput)})`;
  }

  /**
   * Format a tool result
   */
  toolResult(result, options = {}) {
    if (result.error) {
      return this.status('error', result.error);
    }

    if (options.summarize && result.success) {
      // Try to summarize common responses
      if (result.customers) {
        return this.status('success', `Found ${result.count || result.customers.length} customers`);
      }
      if (result.orders) {
        return this.status('success', `Found ${result.totalCount || result.orders.length} orders`);
      }
      if (result.customer) {
        return this.status('success', `Customer: ${result.customer.email}`);
      }
      if (result.order) {
        return this.status(
          'success',
          `Order: ${result.order.orderNumber} (${result.order.status})`,
        );
      }
      if (result.cart) {
        return this.status('success', `Cart: ${result.cart.cartNumber || result.cart.id}`);
      }
      return this.status('success', result.message || 'Done');
    }

    return JSON.stringify(result, null, 2);
  }
}

// ============================================================================
// Structured Output Formatting (File/Programmatic Output)
// ============================================================================

function normalizeScalar(value) {
  if (value === null || value === undefined) return '';
  if (typeof value === 'string') return value;
  if (typeof value === 'number' || typeof value === 'boolean') return String(value);
  return JSON.stringify(value);
}

function normalizeRowsForTable(data) {
  if (Array.isArray(data)) {
    if (data.length === 0) {
      return { rows: [], columns: [] };
    }
    const first = data[0];
    if (first && typeof first === 'object' && !Array.isArray(first)) {
      const keys = Array.from(new Set(data.flatMap((row) => Object.keys(row || {}))));
      const rows = data.map((row) => {
        const out = {};
        for (const key of keys) {
          out[key] = normalizeScalar(row?.[key]);
        }
        return out;
      });
      const columns = keys.map((key) => ({ key, header: key }));
      return { rows, columns };
    }
    const rows = data.map((value) => ({ value: normalizeScalar(value) }));
    return { rows, columns: [{ key: 'value', header: 'value' }] };
  }

  if (data && typeof data === 'object') {
    const rows = Object.entries(data).map(([key, value]) => ({
      key,
      value: normalizeScalar(value),
    }));
    return {
      rows,
      columns: [
        { key: 'key', header: 'key' },
        { key: 'value', header: 'value' },
      ],
    };
  }

  return {
    rows: [{ value: normalizeScalar(data) }],
    columns: [{ key: 'value', header: 'value' }],
  };
}

function formatAsTable(data) {
  const { rows, columns } = normalizeRowsForTable(data);
  const output = new RichOutput({ color: false, format: 'pretty' });
  if (!rows.length) {
    return output.dim('  (no data)');
  }
  return output.table(rows, columns);
}

function formatAsCsv(data) {
  const { rows, columns } = normalizeRowsForTable(data);
  if (!rows.length) return '';
  const headers = columns.map((col) => col.header);
  const keys = columns.map((col) => col.key);
  const lines = rows.map((row) => keys.map((key) => JSON.stringify(row[key] ?? '')).join(','));
  return [headers.join(','), ...lines].join('\n');
}

function formatYamlValue(value) {
  if (value === null || value === undefined) return 'null';
  if (typeof value === 'string') {
    return /[:#\-\n]/.test(value) ? JSON.stringify(value) : value;
  }
  if (typeof value === 'number' || typeof value === 'boolean') return String(value);
  return JSON.stringify(value);
}

function formatAsYaml(data) {
  if (data === null || data === undefined) return '';
  if (Array.isArray(data)) {
    return data.map((item) => `- ${formatYamlValue(item)}`).join('\n');
  }
  if (typeof data === 'object') {
    return Object.entries(data)
      .map(([key, value]) => `${key}: ${formatYamlValue(value)}`)
      .join('\n');
  }
  return String(data);
}

/**
 * Format structured data for file or programmatic output.
 *
 * @param {any} data
 * @param {'table'|'json'|'csv'|'yaml'} format
 * @returns {string}
 */
export function formatStructuredOutput(data, format = 'table') {
  switch (format) {
    case 'json':
      return JSON.stringify(data, null, 2);
    case 'csv':
      return formatAsCsv(data);
    case 'yaml':
      return formatAsYaml(data);
    case 'table':
    default:
      return formatAsTable(data);
  }
}

// ============================================================================
// Convenience Functions
// ============================================================================

/**
 * Create a RichOutput instance with defaults
 */
export function createOutput(options = {}) {
  return new RichOutput(options);
}

/**
 * Default output instance
 */
export const output = new RichOutput();

export default RichOutput;
