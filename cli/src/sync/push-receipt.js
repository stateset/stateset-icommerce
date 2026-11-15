/**
 * Settle every submitted event against a VES ingest response, before any
 * outbox write.
 *
 * The response shape is stateset-sequencer's `VesIngestResponse`
 * (src/api/types.rs, src/infra/postgres/ves_sequencer.rs):
 *
 *  - `rejections` are serialized `{ event_id, reason, message }`: snake_case
 *    inside an otherwise camelCase body;
 *  - `receipts` carry `{ eventId, sequenceNumber, ... }`, one per event that
 *    is now durably sequenced: each newly accepted event, AND each exact
 *    replay of an already-sequenced event (a retry after a lost response).
 *    A replay gets its stored receipt back and is counted in NEITHER
 *    `eventsAccepted` nor `eventsRejected`, so the counts alone cannot say
 *    what happened to it;
 *  - newly accepted events take consecutive sequence numbers
 *    `sequenceStart..sequenceEnd`, in submission order.
 *
 * So the receipts are authoritative: every submitted event must have exactly
 * one receipt or exactly one rejection. A receipt-less response is settled
 * from the range alone, and only when the counts account for every event
 * (no replays): the range then determines each sequence number exactly.
 *
 * Anything inconsistent throws, leaving the whole batch pending for a retry;
 * a retry of events the sequencer did accept comes back as replays and
 * settles.
 *
 * @param {object} receipt - `SequencerClient#push` result
 * @param {Array<{eventId: string}>} pending - the submitted outbox events, in submission order
 * @returns {Array<{event: object, rejected: true, reason: string|number, message: string|null}
 *   | {event: object, rejected: false, remoteSeq: number}>}
 */
export function validatePushReceipt(receipt, pending) {
  const fail = () => {
    throw new Error('Invalid push receipt: incomplete or inconsistent batch acknowledgement');
  };
  const integer = (value) => Number.isSafeInteger(value) && value >= 0;
  if (
    !receipt ||
    !integer(receipt.eventsAccepted) ||
    !integer(receipt.eventsRejected) ||
    !integer(receipt.headSequence)
  )
    fail();
  const submitted = new Map(pending.map((event) => [event.eventId, event]));
  if (submitted.size !== pending.length) fail();

  const rejections = receipt.rejections ?? [];
  if (!Array.isArray(rejections) || rejections.length !== receipt.eventsRejected) fail();
  const rejected = new Map();
  for (const rejection of rejections) {
    const eventId = rejection?.event_id ?? rejection?.eventId;
    if (
      !submitted.has(eventId) ||
      rejected.has(eventId) ||
      !(
        (typeof rejection.reason === 'string' && rejection.reason.trim()) ||
        integer(rejection.reason)
      )
    )
      fail();
    rejected.set(eventId, {
      reason: rejection.reason,
      message: typeof rejection.message === 'string' ? rejection.message : null,
    });
  }

  const accepted = receipt.eventsAccepted;
  const { sequenceStart: start, sequenceEnd: end } = receipt;
  if (
    accepted > 0 &&
    (!integer(start) ||
      start < 1 ||
      !integer(end) ||
      end < start ||
      end > receipt.headSequence ||
      end - start + 1 !== accepted)
  )
    fail();

  const receipts = receipt.receipts ?? [];
  if (!Array.isArray(receipts)) fail();

  if (receipts.length === 0) {
    // Without receipts nothing can stand for a replay, so the counts must
    // cover the batch and the contiguous range maps the rest in order.
    if (accepted + receipt.eventsRejected !== pending.length) fail();
    let sequence = start;
    return pending.map((event) =>
      rejected.has(event.eventId)
        ? { event, rejected: true, ...rejected.get(event.eventId) }
        : { event, rejected: false, remoteSeq: sequence++ },
    );
  }

  const sequenced = new Map();
  const sequenceNumbers = new Set();
  let newlyAccepted = 0;
  for (const entry of receipts) {
    const eventId = entry?.eventId ?? entry?.event_id;
    const seq = entry?.sequenceNumber ?? entry?.sequence_number;
    if (
      !submitted.has(eventId) ||
      sequenced.has(eventId) ||
      rejected.has(eventId) ||
      !integer(seq) ||
      seq < 1 ||
      seq > receipt.headSequence ||
      sequenceNumbers.has(seq)
    )
      fail();
    if (accepted > 0 && seq >= start && seq <= end) {
      newlyAccepted += 1;
    } else if (accepted > 0 && seq > end) {
      fail();
    }
    // A replay's sequence predates this batch's range: it was sequenced by
    // an earlier request before this one took the stream's sequence lock.
    sequenced.set(eventId, seq);
    sequenceNumbers.add(seq);
  }
  if (newlyAccepted !== accepted) fail();
  for (const event of pending) {
    if (!sequenced.has(event.eventId) && !rejected.has(event.eventId)) fail();
  }

  return pending.map((event) =>
    rejected.has(event.eventId)
      ? { event, rejected: true, ...rejected.get(event.eventId) }
      : { event, rejected: false, remoteSeq: sequenced.get(event.eventId) },
  );
}
