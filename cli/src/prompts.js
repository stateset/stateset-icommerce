/**
 * Interactive Prompts Module for StateSet CLI
 *
 * Provides user-friendly prompts for missing required data,
 * confirmations, and interactive input.
 */

import * as readline from 'node:readline';

/**
 * Create a readline interface for user input
 */
function createInterface() {
  return readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });
}

/**
 * Prompt for a single value
 * @param {string} question - The question to ask
 * @param {Object} options - Options (default, validate, transform)
 * @returns {Promise<string>} User input
 */
export async function prompt(question, options = {}) {
  const rl = createInterface();

  return new Promise((resolve) => {
    const defaultHint = options.default ? ` (${options.default})` : '';
    const requiredHint = options.required ? ' *' : '';

    const ask = () => {
      rl.question(`${question}${defaultHint}${requiredHint}: `, async (answer) => {
        let value = answer.trim() || options.default || '';

        // Transform if provided
        if (options.transform) {
          value = options.transform(value);
        }

        // Validate if provided
        if (options.validate) {
          const validation = await options.validate(value);
          if (validation !== true) {
            console.warn(`  \x1b[31m${validation || 'Invalid input'}\x1b[0m`);
            ask();
            return;
          }
        }

        // Check required
        if (options.required && !value) {
          console.warn('  \x1b[31mThis field is required\x1b[0m');
          ask();
          return;
        }

        rl.close();
        resolve(value);
      });
    };

    ask();
  });
}

/**
 * Prompt for confirmation (yes/no)
 * @param {string} question - The question to ask
 * @param {boolean} defaultValue - Default value (true = yes)
 * @returns {Promise<boolean>}
 */
export async function confirm(question, defaultValue = false) {
  const hint = defaultValue ? '[Y/n]' : '[y/N]';
  const answer = await prompt(`${question} ${hint}`);

  if (!answer) return defaultValue;
  return answer.toLowerCase().startsWith('y');
}

/**
 * Prompt for selection from a list
 * @param {string} question - The question to ask
 * @param {Array} choices - Array of choices (strings or {value, label})
 * @returns {Promise<string>} Selected value
 */
export async function select(question, choices) {
  process.stderr.write(`\n${question}\n\n`);

  choices.forEach((choice, i) => {
    const label = typeof choice === 'string' ? choice : choice.label;
    process.stderr.write(`  ${i + 1}) ${label}\n`);
  });

  const answer = await prompt('\nSelect option', {
    validate: (v) => {
      const num = parseInt(v, 10);
      if (isNaN(num) || num < 1 || num > choices.length) {
        return `Please enter a number between 1 and ${choices.length}`;
      }
      return true;
    },
  });

  const index = parseInt(answer, 10) - 1;
  const choice = choices[index];
  return typeof choice === 'string' ? choice : choice.value;
}

/**
 * Prompt for multiple values with schema
 * @param {Object} schema - Schema defining fields
 * @returns {Promise<Object>} Collected values
 */
export async function promptSchema(schema) {
  const result = {};

  for (const [key, field] of Object.entries(schema)) {
    const question = field.description || key;

    if (field.type === 'boolean') {
      result[key] = await confirm(question, field.default);
    } else if (field.enum) {
      result[key] = await select(question, field.enum);
    } else {
      result[key] = await prompt(question, {
        default: field.default,
        required: field.required,
        validate: field.validate,
        transform: field.type === 'number' ? (v) => parseFloat(v) : undefined,
      });
    }
  }

  return result;
}

/**
 * Interactive prompts for common CLI operations
 */
export const InteractivePrompts = {
  /**
   * Prompt for customer creation if fields are missing
   */
  async customer(partial = {}) {
    return {
      email:
        partial.email ||
        (await prompt('Customer email', {
          required: true,
          validate: (v) => v.includes('@') || 'Please enter a valid email',
        })),
      firstName: partial.firstName || (await prompt('First name', { required: true })),
      lastName: partial.lastName || (await prompt('Last name', { required: true })),
      phone: partial.phone || (await prompt('Phone (optional)')),
      acceptsMarketing: partial.acceptsMarketing ?? (await confirm('Accepts marketing?', false)),
    };
  },

  /**
   * Prompt for order creation if fields are missing
   */
  async order(partial = {}) {
    const result = { ...partial };

    if (!result.customerId) {
      result.customerId = await prompt('Customer ID', { required: true });
    }

    if (!result.items || result.items.length === 0) {
      console.info('\nAdd order items (empty SKU to finish):');
      result.items = [];

      while (true) {
        const sku = await prompt('  SKU');
        if (!sku) break;

        const name = await prompt('  Product name', { required: true });
        const quantity = await prompt('  Quantity', {
          default: '1',
          transform: (v) => parseInt(v, 10),
        });
        const unitPrice = await prompt('  Unit price', {
          required: true,
          transform: (v) => parseFloat(v),
        });

        result.items.push({ sku, name, quantity, unitPrice });
        console.info('  Item added.\n');
      }
    }

    result.currency = partial.currency || (await prompt('Currency', { default: 'USD' }));

    return result;
  },

  /**
   * Prompt for inventory adjustment if fields are missing
   */
  async inventoryAdjust(partial = {}) {
    return {
      sku: partial.sku || (await prompt('SKU', { required: true })),
      quantity:
        partial.quantity ??
        (await prompt('Adjustment quantity (+/-)', {
          required: true,
          transform: (v) => parseInt(v, 10),
          validate: (v) => !isNaN(v) || 'Please enter a valid number',
        })),
      reason: partial.reason || (await prompt('Reason for adjustment', { required: true })),
    };
  },

  /**
   * Prompt for return creation if fields are missing
   */
  async returnRequest(partial = {}) {
    const reasons = [
      { value: 'defective', label: 'Defective product' },
      { value: 'wrong_item', label: 'Wrong item received' },
      { value: 'not_as_described', label: 'Not as described' },
      { value: 'no_longer_needed', label: 'No longer needed' },
      { value: 'other', label: 'Other' },
    ];

    return {
      orderId: partial.orderId || (await prompt('Order ID', { required: true })),
      reason: partial.reason || (await select('Return reason', reasons)),
      items: partial.items || [], // Could add interactive item selection
    };
  },

  /**
   * Confirm high-value operation
   */
  async confirmHighValue(operation, details) {
    console.warn(`\n\x1b[33mHigh-value operation: ${operation}\x1b[0m`);

    for (const [key, value] of Object.entries(details)) {
      console.warn(`  ${key}: ${value}`);
    }

    return confirm('\nProceed with this operation?', false);
  },

  /**
   * Confirm destructive operation
   */
  async confirmDestructive(operation, identifier) {
    console.warn(`\n\x1b[31mDestructive operation: ${operation}\x1b[0m`);
    console.warn(`  Target: ${identifier}`);
    console.warn('  This action cannot be undone.\n');

    const confirmation = await prompt('Type the identifier to confirm');
    return confirmation === identifier;
  },
};

/**
 * Check if we're in interactive mode (TTY)
 */
export function isInteractive() {
  return process.stdin.isTTY && process.stdout.isTTY;
}

/**
 * Wrapper to make prompts optional based on TTY
 * Falls back to error if not interactive and value is required
 */
export async function interactiveOr(promptFn, fallbackValue, errorMessage) {
  if (isInteractive()) {
    return promptFn();
  }

  if (fallbackValue !== undefined) {
    return fallbackValue;
  }

  throw new Error(errorMessage || 'Missing required input in non-interactive mode');
}

export default {
  prompt,
  confirm,
  select,
  promptSchema,
  InteractivePrompts,
  isInteractive,
  interactiveOr,
};
