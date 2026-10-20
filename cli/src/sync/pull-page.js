export function validatePullRequest(fromSequence, limit) {
  if (
    !Number.isSafeInteger(fromSequence) ||
    fromSequence < 0 ||
    !Number.isSafeInteger(limit) ||
    limit < 1
  ) {
    throw new Error(
      'Invalid pull request: cursor and limit must be safe nonnegative integers, with a positive limit',
    );
  }
}

// The REST `from` parameter is inclusive. Gaps may belong to other stores;
// never require contiguity or infer progress from the remote head alone.
export function validatePullPage(page, fromSequence, limit) {
  validatePullRequest(fromSequence, limit);
  if (!page || !Array.isArray(page.events) || page.events.length > limit) {
    throw new Error('Invalid pull page: expected an event array within the requested limit');
  }
  let max = fromSequence - 1;
  const seen = new Set();
  for (const event of page.events) {
    const seq = event?.sequenceNumber;
    if (
      !Number.isSafeInteger(seq) ||
      seq < fromSequence ||
      seq >= Number.MAX_SAFE_INTEGER ||
      seen.has(seq)
    ) {
      throw new Error('Invalid pull page: duplicate, stale, or unsafe event sequence');
    }
    seen.add(seq);
    max = Math.max(max, seq);
  }
  const expectedNext = page.events.length ? max + 1 : fromSequence;
  if (
    page.nextSequence !== expectedNext ||
    !Number.isSafeInteger(page.headSequence) ||
    page.headSequence < 0 ||
    (page.events.length && page.headSequence < max) ||
    (page.hasMore !== undefined && typeof page.hasMore !== 'boolean') ||
    (page.events.length === 0 && page.hasMore === true)
  ) {
    throw new Error('Invalid pull page: inconsistent cursor, head, or continuation');
  }
}
