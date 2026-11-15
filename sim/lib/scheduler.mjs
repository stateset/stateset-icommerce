/**
 * Seeded interleaving of actors inside one process.
 *
 * Each turn the scheduler draws a random ready actor and advances it by one
 * step. With `inflight = 1` exactly one step is outstanding at a time (a
 * concurrent-duplicate burst is the one exception: its two identical submits
 * go out together), so the global order of steps -- and therefore every
 * engine outcome -- is a pure function of the seed. With `inflight > 1` the
 * scheduler keeps that many actors' steps outstanding at once;
 * `executeKernelCommand` is an async napi function whose work runs on the
 * binding's tokio worker threads, so those steps genuinely overlap inside the
 * engine (pool checkout, BEGIN IMMEDIATE, busy retries) and outcomes are no
 * longer deterministic.
 */
export async function runActors({ actors, rng, inflight, execute }) {
  const ready = [...actors];
  const running = new Set();

  async function advance(actor) {
    let next;
    try {
      next = actor.gen.next(actor.lastResult);
    } catch (error) {
      actor.done = true;
      actor.record.outcome = 'crashed';
      actor.record.thrown.push({
        label: 'journey',
        kind: 'harness',
        code: null,
        message: String(error?.stack ?? error),
      });
      return;
    }
    if (next.done) {
      actor.done = true;
      return;
    }
    actor.lastResult = await execute(actor, next.value);
  }

  while (ready.length > 0 || running.size > 0) {
    while (ready.length > 0 && running.size < inflight) {
      const slot = Math.floor(rng.next() * ready.length);
      const actor = ready[slot];
      ready[slot] = ready[ready.length - 1];
      ready.pop();
      const task = advance(actor).then(() => {
        running.delete(task);
        if (!actor.done) ready.push(actor);
      });
      running.add(task);
    }
    if (running.size > 0) await Promise.race(running);
  }
}
