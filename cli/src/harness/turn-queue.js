/**
 * Prioritized input queue for createAgentStreamSession turns.
 *
 * Encapsulates the steer / follow-up / plain queues plus the in-turn and
 * closed flags. Extracted from claude-harness.js — all state is per-instance;
 * there is no module-scope state.
 *
 * Ordering semantics preserved exactly:
 * - steered messages win, then follow-ups, then plain sends;
 * - nothing is dequeued while a turn is in flight;
 * - nextMessage() resolves null once the queue is closed.
 */
export function createTurnQueue() {
  const queue = [];
  const followUpQueue = [];
  const steerQueue = [];
  let inTurn = false;
  let closed = false;
  let wakeInput = null;

  const notify = () => {
    if (wakeInput) {
      wakeInput();
      wakeInput = null;
    }
  };

  const enqueue = (text, mode = 'followUp') => {
    if (!text) return;
    if (mode === 'steer') {
      steerQueue.push(text);
    } else if (mode === 'followUp') {
      followUpQueue.push(text);
    } else {
      queue.push(text);
    }
    notify();
  };

  const nextMessage = async () => {
    while (!closed) {
      if (steerQueue.length > 0 && !inTurn) return steerQueue.shift();
      if (!inTurn && followUpQueue.length > 0) return followUpQueue.shift();
      if (!inTurn && queue.length > 0) return queue.shift();
      await new Promise((resolve) => {
        wakeInput = resolve;
      });
    }
    return null;
  };

  return {
    enqueue,
    nextMessage,
    notify,
    setInTurn: (value) => {
      inTurn = value;
    },
    close: () => {
      closed = true;
      notify();
    },
    isClosed: () => closed,
  };
}
