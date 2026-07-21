/**
 * Units-of-measure API tests for @stateset/embedded Node.js bindings.
 *
 * Unit class, UOM, and conversion rule lifecycle.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { randomUUID } = require('node:crypto');

test('UnitsOfMeasure: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('API exists and is supported', async () => {
    assert.ok(commerce.unitsOfMeasure, 'unitsOfMeasure API should exist');
    assert.equal(await commerce.unitsOfMeasure.isSupported(), true);
  });

  let unitClass;
  await t.test('createClass / listClasses', async () => {
    unitClass = await commerce.unitsOfMeasure.createClass({
      name: 'Weight',
      description: 'Mass units',
    });
    assert.ok(unitClass.id);
    assert.equal(unitClass.name, 'Weight');
    assert.ok(unitClass.createdAt);

    const classes = await commerce.unitsOfMeasure.listClasses();
    assert.ok(classes.some((c) => c.id === unitClass.id));
  });

  let gram;
  let kilo;
  await t.test('createUom returns exact-decimal factors', async () => {
    gram = await commerce.unitsOfMeasure.createUom({
      unitClassId: unitClass.id,
      name: 'Gram',
      abbreviation: 'g',
      factor: '1',
    });
    kilo = await commerce.unitsOfMeasure.createUom({
      unitClassId: unitClass.id,
      name: 'Kilogram',
      abbreviation: 'kg',
      factor: '1000.5',
    });
    assert.equal(gram.factor, '1');
    assert.equal(kilo.factor, '1000.5');
    assert.equal(kilo.unitClassId, unitClass.id);
  });

  await t.test('createUom rejects a bad decimal factor', async () => {
    await assert.rejects(
      () =>
        commerce.unitsOfMeasure.createUom({
          unitClassId: unitClass.id,
          name: 'Bad',
          abbreviation: 'b',
          factor: 'not-a-number',
        }),
      /Invalid factor decimal/,
    );
  });

  await t.test('listUoms accepts no filter and a class filter', async () => {
    const all = await commerce.unitsOfMeasure.listUoms();
    assert.ok(all.length >= 2);

    const scoped = await commerce.unitsOfMeasure.listUoms({ classId: unitClass.id, limit: 1 });
    assert.equal(scoped.length, 1);

    const other = await commerce.unitsOfMeasure.listUoms({ classId: randomUUID() });
    assert.equal(other.length, 0);
  });

  await t.test('setBaseUom marks the base unit', async () => {
    const base = await commerce.unitsOfMeasure.setBaseUom(gram.id);
    assert.equal(base.id, gram.id);
    assert.equal(base.isBase, true);
  });

  let rule;
  await t.test('createRule / listRules', async () => {
    rule = await commerce.unitsOfMeasure.createRule({
      ruleType: 'SYSTEM',
      fromUomId: kilo.id,
      toUomId: gram.id,
      factor: '1000',
    });
    assert.equal(rule.ruleType, 'SYSTEM');
    assert.equal(rule.productId, undefined);
    assert.equal(rule.factor, '1000');

    const rules = await commerce.unitsOfMeasure.listRules();
    assert.ok(rules.some((r) => r.id === rule.id));
  });

  await t.test('createRule rejects an invalid rule type', async () => {
    await assert.rejects(
      () =>
        commerce.unitsOfMeasure.createRule({
          ruleType: 'nope',
          fromUomId: kilo.id,
          toUomId: gram.id,
          factor: '1',
        }),
      /Invalid conversion rule type/,
    );
  });

  await t.test('delete removes rules, uoms, and classes', async () => {
    await commerce.unitsOfMeasure.deleteRule(rule.id);
    assert.equal((await commerce.unitsOfMeasure.listRules()).length, 0);

    await commerce.unitsOfMeasure.deleteUom(kilo.id);
    const remaining = await commerce.unitsOfMeasure.listUoms({ classId: unitClass.id });
    assert.ok(!remaining.some((u) => u.id === kilo.id));
  });
});
