package com.stateset.embedded;

/**
 * Cross-binding cryptographic primitives.
 *
 * <p>Thin wrappers over the JNI exports in {@code bindings/java/src/lib.rs}
 * that delegate to the {@code stateset-crypto} Rust crate. The same set of
 * primitives is exported from every StateSet binding and verified against
 * the language-neutral test corpus at
 * {@code bindings/test-vectors/v1.json}.
 *
 * <p>Each method is a static call and threadsafe — there is no class state.
 * On invalid input or runtime failure, methods throw
 * {@link StateSetException}.
 */
public final class Crypto {

    static {
        NativeLoader.load();
    }

    private Crypto() {}

    /**
     * Return the RFC 8785 JCS canonical-form bytes for a JSON string.
     *
     * <p>Callers typically SHA-256 the result themselves when comparing
     * against ground-truth digests.
     *
     * @param json a JSON value as a UTF-8 string
     * @return the canonical UTF-8 bytes
     * @throws StateSetException if {@code json} is not valid JSON or the
     *         canonicalizer fails
     */
    public static byte[] jcsCanonicalize(String json) {
        return nativeJcsCanonicalize(json);
    }

    /**
     * Compute the VES v1.0 payload-plain hash of a JSON payload.
     *
     * <p>Equivalent to
     * {@code sha256(domain.PAYLOAD_PLAIN || optional_salt || jcs(payload))}.
     * The domain prefix matches
     * {@code crates/stateset-crypto/src/lib.rs::domain::PAYLOAD_PLAIN}.
     *
     * @param json a JSON payload as a UTF-8 string
     * @param salt 16 bytes of salt, or {@code null} for unsalted
     * @return the 32-byte digest
     * @throws StateSetException if {@code json} is invalid or {@code salt}
     *         is not exactly 16 bytes
     */
    public static byte[] payloadPlainHash(String json, byte[] salt) {
        return nativePayloadPlainHash(json, salt);
    }

    /**
     * Compute the merkle root of a list of 32-byte leaves.
     *
     * <p>An empty list yields the empty-tree sentinel from
     * {@code stateset-crypto}. Each leaf must be exactly 32 bytes.
     *
     * @param leaves the leaf digests (each 32 bytes)
     * @return the 32-byte merkle root
     * @throws StateSetException if any leaf is not exactly 32 bytes
     */
    public static byte[] merkleRoot(byte[][] leaves) {
        return nativeMerkleRoot(leaves);
    }

    private static native byte[] nativeJcsCanonicalize(String json);
    private static native byte[] nativePayloadPlainHash(String json, byte[] salt);
    private static native byte[] nativeMerkleRoot(byte[][] leaves);
}
