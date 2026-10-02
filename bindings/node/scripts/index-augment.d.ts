// ---- BEGIN hand-written additions (bindings/node/scripts/index-augment.d.ts) ----
// Appended to the napi-generated declarations by scripts/postbuild-types.mjs,
// which every `npm run build*` script runs. These describe the JavaScript-side
// surface index.js layers over the native classes; TypeScript merges each
// interface into the class of the same name.

/**
 * A commerce event as delivered to a subscriber: the serialised
 * `CommerceEvent` (tagged by `type`) plus a stable snake_case `event_type`.
 */
export interface CommerceEvent {
  event_type: string
  type?: string
  [key: string]: unknown
}

export interface CommerceEventSubscription extends AsyncIterable<CommerceEvent> {
  /**
   * The next event, or `null` once the stream has ended (after `close()`, or
   * when the owning `Commerce` closes).
   *
   * An open subscription never keeps the process alive by itself; call
   * `ref()` if it should.
   */
  recv(): Promise<CommerceEvent | null>
  /** Iterate events until the stream ends. Leaving the loop calls `close()`. */
  [Symbol.asyncIterator](): AsyncIterator<CommerceEvent>
}
// ---- END hand-written additions ----
