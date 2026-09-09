// Durable coordination only. Host callbacks own EIP-712/transaction encoding,
// constrained signing, transaction validation, RPC and deployed-contract policy.
const HASH = /^0x[0-9a-fA-F]{64}$/;
const ADDRESS = /^0x[0-9a-fA-F]{40}$/;
const MAX_NONCE = (1n << 64n) - 1n;
function canonical(value) {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
  if (value && typeof value === 'object')
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`)
      .join(',')}}`;
  const result = JSON.stringify(value);
  if (result === undefined) throw new Error('submission must be JSON serializable');
  return result;
}
function uint(value, max) {
  if (
    typeof value !== 'string' ||
    !/^(0|[1-9]\d*)$/.test(value) ||
    value.length > 78 ||
    BigInt(value) > max
  )
    throw new Error('invalid unsigned decimal');
  return value;
}
function checkedIntent(input) {
  const fields = [
    'chainId',
    'settlementContract',
    'payer',
    'payee',
    'token',
    'amount',
    'validUntil',
    'intentId',
    'idempotencyKey',
  ];
  if (
    !input ||
    Object.keys(input).length !== fields.length ||
    Object.keys(input).some((key) => !fields.includes(key))
  )
    throw new Error('invalid submission intent fields');
  const intent = structuredClone(input);
  for (const key of ['settlementContract', 'payer', 'payee', 'token'])
    if (
      typeof intent[key] !== 'string' ||
      !ADDRESS.test(intent[key]) ||
      /^0x0{40}$/.test(intent[key])
    )
      throw new Error('invalid submission address');
  for (const key of ['chainId', 'amount']) {
    uint(intent[key], (1n << 256n) - 1n);
    if (intent[key] === '0') throw new Error('zero chain or amount');
  }
  uint(intent.validUntil, MAX_NONCE);
  if (
    !HASH.test(intent.intentId) ||
    typeof intent.idempotencyKey !== 'string' ||
    !intent.idempotencyKey ||
    intent.idempotencyKey.length > 512
  )
    throw new Error('invalid submission identity');
  return intent;
}

/** Callbacks must not broadcast from prepare/sign/validateSigned. Only broadcast
 * may submit bytes. prepare must pin the relayer transaction nonce; sign must be
 * safe to repeat for the same plan. validateSigned verifies ALL encoded terms,
 * including both nonces, and returns the independently computed transaction hash.
 */
export function createDurableSetSubmission({
  db,
  scope,
  nonceStart,
  prepare,
  sign,
  validateSigned,
  broadcast,
  authorize,
  allowSubmit = false,
  clock = Date.now,
}) {
  if (typeof scope !== 'string' || !scope || scope.length > 512)
    throw new Error('submission scope required');
  uint(nonceStart, MAX_NONCE);
  if (!db?.prepare || !db?.transaction || !db?.exec) throw new Error('SQLite database required');
  for (const fn of [prepare, sign, validateSigned, broadcast, authorize, clock])
    if (typeof fn !== 'function') throw new Error('operator submission callbacks required');
  db.exec(`
    CREATE TABLE IF NOT EXISTS _stateset_set_nonces (
      domain TEXT PRIMARY KEY, initial_nonce TEXT NOT NULL, next_nonce TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS _stateset_set_submissions (
      scope TEXT NOT NULL, request_key TEXT NOT NULL, intent_id TEXT NOT NULL UNIQUE,
      intent TEXT NOT NULL, payer_nonce TEXT NOT NULL, plan TEXT, artifact TEXT,
      acknowledged INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(scope, request_key)
    );
  `);
  const read = (key) =>
    db
      .prepare('SELECT * FROM _stateset_set_submissions WHERE scope=? AND request_key=?')
      .get(scope, key);
  function existing(intent) {
    const row = read(intent.idempotencyKey);
    if (row && row.intent !== canonical(intent)) throw new Error('submission idempotency conflict');
    return row;
  }
  function ensure(intent) {
    return db
      .transaction(() => {
        const prior = existing(intent);
        if (prior) return prior;
        // Share the payer nonce namespace across principals using this journal.
        const domain = canonical([
          intent.chainId,
          intent.settlementContract.toLowerCase(),
          intent.payer.toLowerCase(),
        ]);
        db.prepare('INSERT OR IGNORE INTO _stateset_set_nonces VALUES (?,?,?)').run(
          domain,
          nonceStart,
          nonceStart,
        );
        const counter = db.prepare('SELECT * FROM _stateset_set_nonces WHERE domain=?').get(domain);
        if (counter.initial_nonce !== nonceStart)
          throw new Error('payer nonce configuration conflict');
        const nonce = BigInt(counter.next_nonce);
        if (nonce > MAX_NONCE) throw new Error('payer nonce space exhausted');
        db.prepare('UPDATE _stateset_set_nonces SET next_nonce=? WHERE domain=?').run(
          (nonce + 1n).toString(),
          domain,
        );
        db.prepare(
          'INSERT INTO _stateset_set_submissions(scope,request_key,intent_id,intent,payer_nonce) VALUES (?,?,?,?,?)',
        ).run(
          scope,
          intent.idempotencyKey,
          intent.intentId.toLowerCase(),
          canonical(intent),
          nonce.toString(),
        );
        return read(intent.idempotencyKey);
      })
      .immediate();
  }
  async function permission(intent) {
    if (allowSubmit !== true) throw new Error('submission is disabled');
    const now = clock();
    if (!Number.isSafeInteger(now) || BigInt(intent.validUntil) * 1000n <= BigInt(now))
      throw new Error('submission intent expired');
    if ((await authorize(structuredClone(intent))) !== true)
      throw new Error('submission not authorized');
  }
  async function verify(intent, row, artifact) {
    if (
      !artifact ||
      typeof artifact.rawTransaction !== 'string' ||
      !/^0x(?:[0-9a-fA-F]{2})+$/.test(artifact.rawTransaction) ||
      artifact.rawTransaction.length > 2_000_002 ||
      !HASH.test(artifact.transactionHash)
    )
      throw new Error('invalid signed transaction artifact');
    const expected = {
      intent: structuredClone(intent),
      payerNonce: row.payer_nonce,
      plan: JSON.parse(row.plan),
    };
    const hash = await validateSigned(expected, artifact.rawTransaction);
    if (
      typeof hash !== 'string' ||
      !HASH.test(hash) ||
      hash.toLowerCase() !== artifact.transactionHash.toLowerCase()
    )
      throw new Error('signed transaction verification failed');
  }
  async function submit(input) {
    const intent = checkedIntent(input);
    existing(intent); // Detect conflicts even if permissions have since changed.
    await permission(intent);
    let row = ensure(intent);
    if (!row.plan) {
      // Concurrent preparers may race, but only one immutable plan is selected.
      // Preparation is not permission to sign or broadcast.
      const plan = canonical(
        await prepare({ intent: structuredClone(intent), payerNonce: row.payer_nonce }),
      );
      db.prepare(
        'UPDATE _stateset_set_submissions SET plan=? WHERE scope=? AND request_key=? AND plan IS NULL',
      ).run(plan, scope, intent.idempotencyKey);
      row = read(intent.idempotencyKey);
    }
    if (!row.artifact) {
      await permission(intent);
      const artifact = await sign({
        intent: structuredClone(intent),
        payerNonce: row.payer_nonce,
        plan: JSON.parse(row.plan),
      });
      await verify(intent, row, artifact);
      const saved = canonical({
        rawTransaction: artifact.rawTransaction,
        transactionHash: artifact.transactionHash.toLowerCase(),
      });
      db.prepare(
        'UPDATE _stateset_set_submissions SET artifact=? WHERE scope=? AND request_key=? AND artifact IS NULL',
      ).run(saved, scope, intent.idempotencyKey);
      row = read(intent.idempotencyKey);
    }
    // Always use the persisted winner, never a losing concurrent signature.
    const artifact = JSON.parse(row.artifact);
    await verify(intent, row, artifact);
    if (!row.acknowledged) {
      await permission(intent);
      const hash = await broadcast(artifact.rawTransaction);
      if (typeof hash !== 'string' || hash.toLowerCase() !== artifact.transactionHash)
        throw new Error('broadcast transaction hash mismatch');
      db.prepare(
        'UPDATE _stateset_set_submissions SET acknowledged=1 WHERE scope=? AND request_key=?',
      ).run(scope, intent.idempotencyKey);
    }
    return artifact.transactionHash;
  }
  return Object.freeze({
    submit,
    async findTransaction(input) {
      const intent = checkedIntent(input);
      const row = existing(intent);
      if (!row?.artifact) return null;
      const artifact = JSON.parse(row.artifact);
      await verify(intent, row, artifact);
      return artifact.transactionHash;
    },
    async recover({ limit = 100, after = '' } = {}) {
      if (allowSubmit !== true) throw new Error('submission is disabled');
      if (!Number.isSafeInteger(limit) || limit < 1 || limit > 1000)
        throw new Error('invalid recovery limit');
      if (typeof after !== 'string') throw new Error('invalid recovery cursor');
      const rows = db
        .prepare(
          'SELECT intent FROM _stateset_set_submissions WHERE scope=? AND acknowledged=0 AND request_key>? ORDER BY request_key LIMIT ?',
        )
        .all(scope, after, limit);
      const results = [];
      for (const row of rows) {
        const intent = JSON.parse(row.intent);
        try {
          results.push({
            idempotencyKey: intent.idempotencyKey,
            transactionHash: await submit(intent),
          });
        } catch (error) {
          results.push({
            idempotencyKey: intent.idempotencyKey,
            error: String(error.message || error),
          });
        }
      }
      return results;
    },
  });
}
