/** Validate the existing ordered, contiguous push receipt contract before writes. */
export function validatePushReceipt(receipt, pending) {
  const fail = () => {
    throw new Error('Invalid push receipt: incomplete or inconsistent batch acknowledgement');
  };
  const integer = (value) => Number.isSafeInteger(value) && value >= 0;
  if (
    !receipt ||
    !integer(receipt.eventsAccepted) ||
    !integer(receipt.eventsRejected) ||
    !integer(receipt.headSequence) ||
    receipt.eventsAccepted + receipt.eventsRejected !== pending.length
  )
    fail();
  const rejections = receipt.rejections ?? [];
  if (!Array.isArray(rejections) || rejections.length !== receipt.eventsRejected) fail();
  const submitted = new Map(pending.map((event) => [event.eventId, event]));
  if (submitted.size !== pending.length) fail();
  const rejected = new Map();
  for (const rejection of rejections) {
    if (
      !rejection ||
      !submitted.has(rejection.eventId) ||
      rejected.has(rejection.eventId) ||
      !(
        (typeof rejection.reason === 'string' && rejection.reason.trim()) ||
        integer(rejection.reason)
      )
    )
      fail();
    rejected.set(rejection.eventId, rejection.reason);
  }
  if (
    receipt.eventsAccepted > 0 &&
    (!integer(receipt.sequenceStart) ||
      receipt.sequenceStart < 1 ||
      !integer(receipt.sequenceEnd) ||
      receipt.sequenceEnd < receipt.sequenceStart ||
      receipt.sequenceEnd > receipt.headSequence ||
      receipt.sequenceEnd - receipt.sequenceStart + 1 !== receipt.eventsAccepted)
  )
    fail();
  let sequence = receipt.sequenceStart;
  return pending.map((event) =>
    rejected.has(event.eventId)
      ? { event, rejected: true, reason: rejected.get(event.eventId) }
      : { event, rejected: false, remoteSeq: sequence++ },
  );
}
