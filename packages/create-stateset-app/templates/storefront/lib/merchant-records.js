import { isDeepStrictEqual } from 'node:util';

const handle = 'storefront_operation';

// Native custom-object handles are unique across processes sharing the store.
export async function readMerchantRecord(commerce, key) {
  const row = await commerce.customObjects.getObjectByHandle(handle, key);
  return row ? JSON.parse(row.valuesJson).data : null;
}

export async function saveMerchantRecord(commerce, key, data, collection = undefined) {
  const records = commerce.customObjects;
  if (!(await records.getTypeByHandle(handle))) {
    try {
      await records.createType({
        handle,
        displayName: 'Storefront operations',
        fields: [{ key: 'data', fieldType: 'json', required: true }],
      });
    } catch (error) {
      if (!(await records.getTypeByHandle(handle))) throw error;
    }
  }
  const valuesJson = JSON.stringify({ data });
  try {
    await records.createObject({
      typeHandle: handle,
      handle: key,
      valuesJson,
      ...(collection && { ownerType: 'storefront', ownerId: collection }),
    });
  } catch (error) {
    const winner = await readMerchantRecord(commerce, key);
    if (!winner || !isDeepStrictEqual(winner, JSON.parse(valuesJson).data)) throw error;
  }
  return data;
}

export async function* merchantRecords(commerce, collection, { includeHandle = false } = {}) {
  const seen = new Set();
  for (let offset = 0; ; offset += 100) {
    const rows = await commerce.customObjects.listObjects({
      typeHandle: handle,
      ownerType: 'storefront',
      ownerId: collection,
      offset,
      limit: 100,
    });
    for (const row of rows) {
      if (seen.has(row.handle)) continue;
      seen.add(row.handle);
      yield {
        key: row.handle,
        ...JSON.parse(row.valuesJson).data,
        ...(includeHandle && { handle: row.handle }),
      };
    }
    if (rows.length < 100) break;
  }
}

export async function firstMerchantRecord(commerce, key, candidate, collection = undefined) {
  const prior = await readMerchantRecord(commerce, key);
  if (prior) return prior;
  try {
    return await saveMerchantRecord(commerce, key, candidate, collection);
  } catch (error) {
    const winner = await readMerchantRecord(commerce, key);
    if (!winner) throw error;
    return winner;
  }
}
