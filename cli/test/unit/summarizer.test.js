/**
 * Tests for cli/src/memory/summarizer.js
 *
 * Covers: ConversationSummarizer._parseResponse, summarize (fallback paths),
 * getSummarizer singleton.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import { ConversationSummarizer, getSummarizer } from '../../src/memory/summarizer.js';

// ---------------------------------------------------------------------------
// _parseResponse
// ---------------------------------------------------------------------------

describe('ConversationSummarizer._parseResponse', () => {
  let summarizer;

  beforeEach(() => {
    summarizer = new ConversationSummarizer({ apiKey: null });
  });

  it('parses SUMMARY + FACTS format', () => {
    const text =
      'SUMMARY: The customer asked about order status.\nFACTS: ["order #123", "customer Alice"]';
    const result = summarizer._parseResponse(text, 100);
    assert.equal(result.summary, 'The customer asked about order status.');
    assert.deepStrictEqual(result.facts, ['order #123', 'customer Alice']);
    assert.equal(result.tokenCount, 100);
  });

  it('handles SUMMARY only', () => {
    const result = summarizer._parseResponse('SUMMARY: Just a summary.', 50);
    assert.equal(result.summary, 'Just a summary.');
    assert.deepStrictEqual(result.facts, []);
  });

  it('handles plain text without markers', () => {
    const result = summarizer._parseResponse('Plain text response', 10);
    assert.equal(result.summary, 'Plain text response');
    assert.deepStrictEqual(result.facts, []);
  });

  it('handles malformed FACTS JSON gracefully', () => {
    const text = 'SUMMARY: Test\nFACTS: [not valid json, but comma separated]';
    const result = summarizer._parseResponse(text, 10);
    assert.equal(result.summary, 'Test');
    assert.ok(result.facts.length > 0);
  });

  it('handles empty FACTS array', () => {
    const text = 'SUMMARY: Nothing happened.\nFACTS: []';
    const result = summarizer._parseResponse(text, 5);
    assert.deepStrictEqual(result.facts, []);
  });
});

// ---------------------------------------------------------------------------
// summarize (fallback paths — no API key)
// ---------------------------------------------------------------------------

describe('ConversationSummarizer.summarize', () => {
  let summarizer;

  beforeEach(() => {
    summarizer = new ConversationSummarizer({ apiKey: null });
  });

  it('returns empty for short/empty input', async () => {
    const result = await summarizer.summarize('');
    assert.equal(result.summary, '');
    assert.deepStrictEqual(result.facts, []);
    assert.equal(result.tokenCount, 0);
  });

  it('returns empty for null input', async () => {
    const result = await summarizer.summarize(null);
    assert.equal(result.summary, '');
  });

  it('returns short text for under 20 chars', async () => {
    const result = await summarizer.summarize('hi there!');
    assert.equal(result.summary, 'hi there!');
  });

  it('returns truncated fallback when no API key', async () => {
    const longText = 'A'.repeat(1000);
    const result = await summarizer.summarize(longText);
    assert.ok(result.summary.length <= 500);
    assert.deepStrictEqual(result.facts, []);
  });
});

// ---------------------------------------------------------------------------
// getSummarizer singleton
// ---------------------------------------------------------------------------

describe('getSummarizer', () => {
  it('returns a ConversationSummarizer instance', () => {
    const s = getSummarizer({ apiKey: null });
    assert.ok(s instanceof ConversationSummarizer);
  });
});
