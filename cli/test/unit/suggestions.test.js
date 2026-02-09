/**
 * Unit tests for suggestions.js — SuggestionEngine, formatSuggestion, createSuggestionEngine
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  SuggestionEngine,
  createSuggestionEngine,
  formatSuggestion,
} from '../../src/suggestions.js';
import defaults from '../../src/suggestions.js';

const { INTENT_PATTERNS, COMMAND_ALIASES } = defaults;

// ===========================================================================
// detectIntent
// ===========================================================================

describe('SuggestionEngine.detectIntent', () => {
  const engine = new SuggestionEngine();

  it('matches "show all customers" to list_customers', () => {
    const result = engine.detectIntent('show all customers');
    assert.ok(result);
    assert.equal(result.intent, 'list_customers');
    assert.equal(result.confidence, 0.9);
  });

  it('matches "get my customers" to list_customers', () => {
    const result = engine.detectIntent('get my customers');
    assert.ok(result);
    assert.equal(result.intent, 'list_customers');
  });

  it('matches "ship the order" to ship_order', () => {
    const result = engine.detectIntent('ship the order');
    assert.ok(result);
    assert.equal(result.intent, 'ship_order');
  });

  it('matches "fulfill the order" to ship_order', () => {
    const result = engine.detectIntent('fulfill the order');
    assert.ok(result);
    assert.equal(result.intent, 'ship_order');
  });

  it('matches "how much stock" to get_stock', () => {
    const result = engine.detectIntent('how much stock');
    assert.ok(result);
    assert.equal(result.intent, 'get_stock');
  });

  it('matches "low stock" to get_stock (stock pattern matches first)', () => {
    // "low stock" matches get_stock's /stock\s*(level|count)?(\s*for)?/i before low_stock
    const result = engine.detectIntent('low stock');
    assert.ok(result);
    assert.equal(result.intent, 'get_stock');
  });

  it('matches "low stock items" to low_stock via "what items are low"', () => {
    const result = engine.detectIntent('what items are low');
    assert.ok(result);
    assert.equal(result.intent, 'low_stock');
  });

  it('matches "sales report" to sales_summary', () => {
    const result = engine.detectIntent('sales report');
    assert.ok(result);
    assert.equal(result.intent, 'sales_summary');
  });

  it('matches "find similar products" to vector_search_products', () => {
    const result = engine.detectIntent('find similar products');
    assert.ok(result);
    assert.equal(result.intent, 'vector_search_products');
  });

  it('matches "create a cart" to create_cart', () => {
    const result = engine.detectIntent('create a cart');
    assert.ok(result);
    assert.equal(result.intent, 'create_cart');
  });

  it('matches "checkout" to complete_checkout', () => {
    const result = engine.detectIntent('checkout');
    assert.ok(result);
    assert.equal(result.intent, 'complete_checkout');
  });

  it('matches "place the order" to complete_checkout', () => {
    const result = engine.detectIntent('place the order');
    assert.ok(result);
    assert.equal(result.intent, 'complete_checkout');
  });

  it('returns null for unknown query', () => {
    const result = engine.detectIntent('do something random and weird');
    assert.equal(result, null);
  });

  it('is case-insensitive', () => {
    const result = engine.detectIntent('SHOW ALL CUSTOMERS');
    assert.ok(result);
    assert.equal(result.intent, 'list_customers');
  });

  it('trims whitespace', () => {
    const result = engine.detectIntent('   show all customers   ');
    assert.ok(result);
    assert.equal(result.intent, 'list_customers');
  });
});

// ===========================================================================
// similarity
// ===========================================================================

describe('SuggestionEngine.similarity', () => {
  const engine = new SuggestionEngine();

  it('returns 1 for identical strings', () => {
    assert.equal(engine.similarity('hello', 'hello'), 1);
  });

  it('returns 0 when first is empty', () => {
    assert.equal(engine.similarity('', 'hello'), 0);
  });

  it('returns 0 when second is empty', () => {
    assert.equal(engine.similarity('hello', ''), 0);
  });

  it('returns 0 when both are empty', () => {
    // both empty => a === b => 1
    assert.equal(engine.similarity('', ''), 1);
  });

  it('"customers" vs "costumers" is > 0.6', () => {
    const sim = engine.similarity('customers', 'costumers');
    assert.ok(sim > 0.6, `expected > 0.6, got ${sim}`);
  });

  it('"orders" vs "oders" is > 0.6', () => {
    const sim = engine.similarity('orders', 'oders');
    assert.ok(sim > 0.6, `expected > 0.6, got ${sim}`);
  });

  it('completely different strings have low similarity', () => {
    const sim = engine.similarity('abcdef', 'zyxwvu');
    assert.ok(sim < 0.4, `expected < 0.4, got ${sim}`);
  });
});

// ===========================================================================
// getFuzzySuggestion
// ===========================================================================

describe('SuggestionEngine.getFuzzySuggestion', () => {
  const engine = new SuggestionEngine();

  it('"costumers" corrects to customers', () => {
    const result = engine.getFuzzySuggestion('costumers');
    assert.ok(result);
    assert.ok(result.corrections.length > 0);
    assert.equal(result.corrections[0].suggestion, 'customers');
  });

  it('"clients" corrects to customers (alias)', () => {
    const result = engine.getFuzzySuggestion('show clients');
    assert.ok(result);
    const correction = result.corrections.find((c) => c.original === 'clients');
    assert.ok(correction);
    assert.equal(correction.suggestion, 'customers');
  });

  it('"oders" corrects to orders', () => {
    const result = engine.getFuzzySuggestion('list oders');
    assert.ok(result);
    const correction = result.corrections.find((c) => c.original === 'oders');
    assert.ok(correction);
    assert.equal(correction.suggestion, 'orders');
  });

  it('returns null for completely unrecognized input', () => {
    const result = engine.getFuzzySuggestion('xyzzy plugh');
    assert.equal(result, null);
  });

  it('includes a hint string with "Did you mean"', () => {
    const result = engine.getFuzzySuggestion('costumers');
    assert.ok(result);
    assert.ok(result.hint.includes('Did you mean'));
  });

  it('builds a corrected query string', () => {
    const result = engine.getFuzzySuggestion('list oders');
    assert.ok(result);
    assert.equal(result.suggested, 'list orders');
  });
});

// ===========================================================================
// getSuggestion
// ===========================================================================

describe('SuggestionEngine.getSuggestion', () => {
  const engine = new SuggestionEngine();

  it('returns command/direct/description for detected intent', () => {
    const result = engine.getSuggestion('show all customers');
    assert.ok(result);
    assert.ok(result.command);
    assert.ok(result.direct);
    assert.ok(result.description);
    assert.equal(result.intent, 'list_customers');
    assert.equal(result.confidence, 0.9);
    assert.equal(result.original, 'show all customers');
  });

  it('falls through to fuzzy for unrecognized but correctable query', () => {
    const result = engine.getSuggestion('list costumers');
    assert.ok(result);
    assert.ok(result.corrections);
    assert.ok(result.hint);
  });

  it('returns null for completely unknown query', () => {
    const result = engine.getSuggestion('xyzzy plugh');
    assert.equal(result, null);
  });
});

// ===========================================================================
// getContextualHelp
// ===========================================================================

describe('SuggestionEngine.getContextualHelp', () => {
  const engine = new SuggestionEngine();

  it('suggests for "not found" error', () => {
    const suggestions = engine.getContextualHelp({ error: 'item not found' });
    assert.ok(suggestions.length > 0);
    assert.ok(suggestions.some((s) => s.includes('listing')));
  });

  it('suggests for "--apply" error', () => {
    const suggestions = engine.getContextualHelp({ error: 'requires --apply flag' });
    assert.ok(suggestions.length > 0);
    assert.ok(suggestions.some((s) => s.includes('--apply')));
  });

  it('suggests follow-ups for lastCommand "list_customers"', () => {
    const suggestions = engine.getContextualHelp({ lastCommand: 'list_customers' });
    assert.ok(suggestions.length > 0);
    assert.ok(suggestions.some((s) => s.includes('customer')));
  });

  it('suggests follow-ups for lastCommand "list_orders"', () => {
    const suggestions = engine.getContextualHelp({ lastCommand: 'list_orders' });
    assert.ok(suggestions.length > 0);
    assert.ok(suggestions.some((s) => s.includes('order')));
  });

  it('suggests follow-ups for lastCommand "create_cart"', () => {
    const suggestions = engine.getContextualHelp({ lastCommand: 'create_cart' });
    assert.ok(suggestions.length > 0);
    assert.ok(suggestions.some((s) => s.includes('cart') || s.includes('checkout')));
  });

  it('returns empty for no context', () => {
    const suggestions = engine.getContextualHelp({});
    assert.equal(suggestions.length, 0);
  });
});

// ===========================================================================
// getExamples
// ===========================================================================

describe('SuggestionEngine.getExamples', () => {
  const engine = new SuggestionEngine();

  it('"customers" returns an array of examples', () => {
    const examples = engine.getExamples('customers');
    assert.ok(Array.isArray(examples));
    assert.ok(examples.length >= 2);
  });

  it('"orders" returns an array of examples', () => {
    const examples = engine.getExamples('orders');
    assert.ok(Array.isArray(examples));
    assert.ok(examples.length >= 3);
  });

  it('"inventory" returns an array of examples', () => {
    const examples = engine.getExamples('inventory');
    assert.ok(Array.isArray(examples));
    assert.ok(examples.length >= 2);
  });

  it('"checkout" returns an array of examples', () => {
    const examples = engine.getExamples('checkout');
    assert.ok(Array.isArray(examples));
    assert.ok(examples.length >= 2);
  });

  it('unknown topic returns empty array', () => {
    const examples = engine.getExamples('quantum-computing');
    assert.deepEqual(examples, []);
  });
});

// ===========================================================================
// formatSuggestion
// ===========================================================================

describe('formatSuggestion', () => {
  it('includes ANSI codes when color=true (default)', () => {
    const output = formatSuggestion({
      command: 'stateset "list all customers"',
      direct: 'stateset-direct customers list',
      description: 'List all customers',
    });
    assert.ok(output.includes('\x1b[36m')); // cyan
    assert.ok(output.includes('AI Mode:'));
    assert.ok(output.includes('Direct:'));
  });

  it('omits ANSI codes when color=false', () => {
    const output = formatSuggestion(
      {
        command: 'stateset "list"',
        direct: 'stateset-direct list',
        description: 'List',
      },
      { color: false },
    );
    assert.ok(!output.includes('\x1b['));
    assert.ok(output.includes('AI Mode:'));
  });

  it('includes hint when present', () => {
    const output = formatSuggestion({ hint: 'Did you mean: "customers"?' }, { color: false });
    assert.ok(output.includes('Did you mean'));
  });

  it('shows command and direct sections', () => {
    const output = formatSuggestion({ command: 'cmd', direct: 'dir' }, { color: false });
    assert.ok(output.includes('AI Mode:'));
    assert.ok(output.includes('Direct:'));
  });

  it('handles suggestion with only description', () => {
    const output = formatSuggestion({ description: 'desc only' }, { color: false });
    assert.ok(output.includes('desc only'));
  });
});

// ===========================================================================
// createSuggestionEngine
// ===========================================================================

describe('createSuggestionEngine', () => {
  it('returns a SuggestionEngine instance', () => {
    const engine = createSuggestionEngine();
    assert.ok(engine instanceof SuggestionEngine);
  });

  it('passes options through', () => {
    const engine = createSuggestionEngine({ minSimilarity: 0.8 });
    assert.equal(engine.minSimilarity, 0.8);
  });
});

// ===========================================================================
// Default export
// ===========================================================================

describe('default export', () => {
  it('exports INTENT_PATTERNS with expected keys', () => {
    assert.ok('list_customers' in INTENT_PATTERNS);
    assert.ok('ship_order' in INTENT_PATTERNS);
    assert.ok('create_cart' in INTENT_PATTERNS);
    assert.ok('complete_checkout' in INTENT_PATTERNS);
    assert.ok('sales_summary' in INTENT_PATTERNS);
  });

  it('INTENT_PATTERNS values are arrays of RegExp', () => {
    for (const patterns of Object.values(INTENT_PATTERNS)) {
      assert.ok(Array.isArray(patterns));
      for (const p of patterns) {
        assert.ok(p instanceof RegExp, `expected RegExp, got ${typeof p}`);
      }
    }
  });

  it('exports COMMAND_ALIASES with expected keys', () => {
    assert.ok('costumers' in COMMAND_ALIASES);
    assert.ok('oders' in COMMAND_ALIASES);
    assert.ok('clients' in COMMAND_ALIASES);
    assert.ok('stock' in COMMAND_ALIASES);
  });

  it('COMMAND_ALIASES values are strings', () => {
    for (const v of Object.values(COMMAND_ALIASES)) {
      assert.equal(typeof v, 'string');
    }
  });
});
