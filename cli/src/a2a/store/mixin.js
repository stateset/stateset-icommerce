/**
 * A2A Store — prototype mixin helper.
 *
 * Copies method descriptors from source classes onto a target class prototype
 * so that domain modules can be authored as plain classes (no trailing commas,
 * verbatim class-body syntax) while `A2AStore` stays a single public class.
 */

/**
 * Copy every own prototype member (except `constructor`) from each source class
 * onto `Target.prototype`, preserving the original property descriptors
 * (non-enumerable, writable, configurable — identical to a class method).
 *
 * @param {Function} Target - Class receiving the methods.
 * @param {...Function} sources - Mixin classes whose prototype members are copied.
 * @returns {Function} Target, for chaining.
 */
export function applyStoreMixins(Target, ...sources) {
  for (const Source of sources) {
    for (const name of Object.getOwnPropertyNames(Source.prototype)) {
      if (name === 'constructor') continue;
      const descriptor = Object.getOwnPropertyDescriptor(Source.prototype, name);
      if (!descriptor) continue;
      Object.defineProperty(Target.prototype, name, descriptor);
    }
  }
  return Target;
}
