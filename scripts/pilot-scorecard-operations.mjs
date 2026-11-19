// Merchant and maintainer review the original records privately. The scorecard retains only
// an anonymous, purpose-bound summary and checks two independently enrolled signing keys.
import {
  createHash,
  createHmac,
  createPublicKey,
  generateKeyPairSync,
  sign,
  verify,
} from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import { parseArgs } from 'node:util';
import { pathToFileURL } from 'node:url';

const schema = 'stateset.embedded-pilot.v1';
const fields = [
  'schema',
  'team',
  'workflow',
  'origin',
  'enrolledAt',
  'afterEnrollment',
  'receiptDay',
  'externalDay',
  'matchingKeyId',
  'receiptTag',
  'externalTag',
];
const workflows = new Set(['order-cancellation', 'customer-refund']);
const digest = /^[a-f0-9]{64}$/;
const label = /^[a-z0-9][a-z0-9_-]{0,47}$/;
const uuid = /^[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/i;
const reasons = {
  merchant: 'merchant-confirmed-own-production-operation',
  maintainer: 'maintainer-inspected-and-matched-both-production-records',
};

function date(value) {
  const t = Date.parse(value);
  if (
    typeof value !== 'string' ||
    !Number.isFinite(t) ||
    new Date(t).toISOString().slice(0, 10) !== value
  )
    throw new Error('Expected a UTC calendar date');
  return value;
}

function timestamp(value) {
  if (
    typeof value !== 'string' ||
    !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?(?:Z|[+-]\d\d:\d\d)$/.test(value) ||
    !Number.isFinite(Date.parse(value))
  )
    throw new Error('Expected a completion timestamp with a timezone');
  date(value.slice(0, 10));
  return new Date(value).toISOString();
}

function exactKeys(object, keys) {
  if (
    !object ||
    typeof object !== 'object' ||
    Array.isArray(object) ||
    Object.keys(object).length !== keys.length ||
    !keys.every((key) => Object.hasOwn(object, key))
  )
    throw new Error('Unexpected or missing private review fields');
}

export function validateWorkflow(value) {
  if (!workflows.has(value)) throw new Error('Unsupported embedded workflow');
}

export function publicKey(encoded) {
  if (typeof encoded !== 'string' || encoded.length !== 60)
    throw new Error('Expected an enrolled Ed25519 public key');
  const key = createPublicKey({ key: Buffer.from(encoded, 'base64'), format: 'der', type: 'spki' });
  if (
    key.asymmetricKeyType !== 'ed25519' ||
    key.export({ format: 'der', type: 'spki' }).toString('base64') !== encoded
  )
    throw new Error('Expected an enrolled Ed25519 public key');
  return key;
}

function normalizedClaim(raw) {
  exactKeys(raw, fields);
  validateWorkflow(raw.workflow);
  if (
    raw.schema !== schema ||
    typeof raw.team !== 'string' ||
    !label.test(raw.team) ||
    raw.origin !== 'live-production' ||
    typeof raw.afterEnrollment !== 'boolean' ||
    raw.enrolledAt !== timestamp(raw.enrolledAt) ||
    ![raw.matchingKeyId, raw.receiptTag, raw.externalTag].every(
      (value) => typeof value === 'string' && digest.test(value),
    )
  )
    throw new Error('Invalid live embedded evidence');
  date(raw.receiptDay);
  date(raw.externalDay);
  return Object.fromEntries(fields.map((key) => [key, raw[key]]));
}

// Reads the actual governed receipt and the completed external operation in the operator session.
// Source identifiers, amounts, customer fields and receipt contents never enter the summary.
export function summarizeOperation(team, workflow, source, matchKey) {
  validateWorkflow(workflow);
  if (!Buffer.isBuffer(matchKey) || matchKey.length !== 32)
    throw new Error('Use the pilot’s 32-byte matching key');
  const { receipt, external } = source;
  const refund = workflow === 'customer-refund';
  const store = receipt?.economic_context?.store_id;
  const testStore =
    /(?:^|[^a-z0-9])(?:test|sandbox|fixture|demo|launch-drill|simulation|staging)(?:$|[^a-z0-9])/i;
  if (
    typeof store !== 'string' ||
    !store.trim() ||
    store !== source.productionStoreId ||
    testStore.test(store) ||
    !digest.test(receipt.audit_hash || '') ||
    !Array.isArray(receipt.event_ids) ||
    !receipt.event_ids.length ||
    !receipt.event_ids.every((id) => typeof id === 'string' && uuid.test(id))
  )
    throw new Error('Require a committed receipt from the reviewed production store');
  if (
    source.provenance !== 'live-merchant-operation' ||
    source.receiptEnvironment !== 'production' ||
    external?.environment !== 'production' ||
    receipt?.status !== 'succeeded' ||
    !uuid.test(receipt.receipt_id || '') ||
    !receipt.result ||
    !uuid.test(receipt.aggregate_id || '') ||
    receipt.result.id !== receipt.aggregate_id ||
    (refund
      ? receipt.command_type !== 'payments.create_refund' || receipt.aggregate_type !== 'refund'
      : receipt.command_type !== 'orders.transition' ||
        receipt.aggregate_type !== 'order' ||
        receipt.result.status !== 'cancelled') ||
    external.outcome !== (refund ? 'refund-completed' : 'order-cancelled')
  )
    throw new Error(
      'Require a matching successful production receipt and completed live external operation',
    );
  if (
    !label.test(external.system || '') ||
    ![external.system, external.account, external.operationId].every(
      (s) => typeof s === 'string' && s.length && s.trim() === s && s.length <= 512,
    )
  )
    throw new Error(
      'Supply stable external system, account and operation identifiers in the private input',
    );
  const tag = (parts) => createHmac('sha256', matchKey).update(JSON.stringify(parts)).digest('hex');
  const enrolledAt = timestamp(source.enrolledAt);
  const receiptAt = timestamp(receipt.completed_at);
  const externalAt = timestamp(external.completedAt);
  return normalizedClaim({
    schema,
    team,
    workflow,
    origin: 'live-production',
    enrolledAt,
    afterEnrollment: receiptAt >= enrolledAt && externalAt >= enrolledAt,
    receiptDay: receiptAt.slice(0, 10),
    externalDay: externalAt.slice(0, 10),
    matchingKeyId: createHash('sha256').update(matchKey).digest('hex'),
    receiptTag: tag(['stateset-receipt', receipt.receipt_id.toLowerCase()]),
    externalTag: tag([
      'external-operation',
      external.system,
      external.account,
      external.operationId,
    ]),
  });
}

function statement(claim, role, reviewedOn) {
  if (!Object.hasOwn(reasons, role)) throw new Error('Use a merchant or maintainer role');
  date(reviewedOn);
  if (reviewedOn < claim.receiptDay || reviewedOn < claim.externalDay)
    throw new Error('Review precedes the operation');
  return Buffer.from(
    JSON.stringify([schema, role, reasons[role], reviewedOn, normalizedClaim(claim)]),
  );
}

export function signReview(claim, role, reviewedOn, privateKey) {
  return {
    claim: normalizedClaim(claim),
    role,
    reviewedOn,
    signature: sign(null, statement(claim, role, reviewedOn), privateKey).toString('base64'),
  };
}

export function checkedReview(row, role, record, authority) {
  exactKeys(record, ['claim', 'role', 'reviewedOn', 'signature']);
  const claim = normalizedClaim(record.claim);
  if (
    claim.team !== row.id ||
    claim.workflow !== row.workflow ||
    claim.enrolledAt !== row.enrolledAt ||
    role !== record.role ||
    authority?.matchingKeyId !== claim.matchingKeyId
  )
    throw new Error('Embedded review does not match the enrolled team, workflow, role or pilot');
  if (
    !row.merchantPublicKey ||
    !authority?.maintainerPublicKey ||
    row.merchantPublicKey === authority.maintainerPublicKey
  )
    throw new Error('Require distinct, independently enrolled merchant and maintainer keys');
  const key = publicKey(
    role === 'merchant' ? row.merchantPublicKey : authority.maintainerPublicKey,
  );
  if (
    typeof record.signature !== 'string' ||
    record.signature.length !== 88 ||
    !verify(
      null,
      statement(claim, role, record.reviewedOn),
      key,
      Buffer.from(record.signature, 'base64'),
    )
  )
    throw new Error('Invalid embedded review signature');
  return claim;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const { positionals, values } = parseArgs({
      allowPositionals: true,
      options: {
        key: { type: 'string' },
        'matching-key': { type: 'string' },
        team: { type: 'string' },
        workflow: { type: 'string' },
        role: { type: 'string' },
        apply: { type: 'boolean' },
      },
    });
    const [command] = positionals;
    if (positionals.length !== 1) throw new Error('Choose keygen, prepare or sign');
    if (command === 'keygen') {
      if (!values.apply || !values.key)
        throw new Error('Use keygen --key NEW_PRIVATE_PATH --apply');
      const pair = generateKeyPairSync('ed25519');
      await writeFile(values.key, pair.privateKey.export({ format: 'pem', type: 'pkcs8' }), {
        flag: 'wx',
        mode: 0o600,
      });
      console.log(pair.publicKey.export({ format: 'der', type: 'spki' }).toString('base64'));
    } else if (command === 'prepare' || command === 'sign') {
      let input = '';
      for await (const chunk of process.stdin) input += chunk;
      let value;
      try {
        value = JSON.parse(input);
      } catch {
        throw new Error('Supply valid private JSON on standard input');
      }
      const output =
        command === 'prepare'
          ? summarizeOperation(
              values.team,
              values.workflow,
              value,
              await readFile(values['matching-key']),
            )
          : signReview(
              value,
              values.role,
              new Date().toISOString().slice(0, 10),
              await readFile(values.key),
            );
      console.log(JSON.stringify(output, null, 2));
    } else throw new Error('Choose keygen, prepare or sign');
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
