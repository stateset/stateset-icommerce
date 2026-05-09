package com.stateset.embedded

/**
 * Cross-binding cryptographic primitives.
 *
 * Thin wrappers over the JNI exports in `bindings/kotlin/src/lib.rs` that
 * delegate to the `stateset-crypto` Rust crate. The same primitives are
 * exported from every StateSet binding and verified against the
 * language-neutral test corpus at `bindings/test-vectors/v1.json`.
 *
 * All methods are thread-safe and have no class state. On invalid input
 * or runtime failure they throw [StateSetException].
 */
object Crypto {

    init {
        NativeLoader.load()
    }

    /**
     * Return the RFC 8785 JCS canonical-form bytes for a JSON string.
     *
     * Callers typically SHA-256 the result themselves when comparing
     * against ground-truth digests.
     *
     * @param json a JSON value as a UTF-8 string
     * @return the canonical UTF-8 bytes
     * @throws StateSetException if [json] is invalid or canonicalization fails
     */
    fun jcsCanonicalize(json: String): ByteArray = nativeJcsCanonicalize(json)

    /**
     * Compute the VES v1.0 payload-plain hash of a JSON payload.
     *
     * Equivalent to
     * `sha256(domain.PAYLOAD_PLAIN || optional_salt || jcs(payload))`.
     * The domain prefix matches
     * `crates/stateset-crypto/src/lib.rs::domain::PAYLOAD_PLAIN`.
     *
     * @param json a JSON payload as a UTF-8 string
     * @param salt 16 bytes of salt, or `null` for unsalted
     * @return the 32-byte digest
     * @throws StateSetException if [json] is invalid or [salt] is not 16 bytes
     */
    fun payloadPlainHash(json: String, salt: ByteArray? = null): ByteArray =
        nativePayloadPlainHash(json, salt)

    /**
     * Compute the merkle root of a list of 32-byte leaves.
     *
     * An empty list yields the empty-tree sentinel from `stateset-crypto`.
     * Each leaf must be exactly 32 bytes.
     *
     * @param leaves the leaf digests (each 32 bytes)
     * @return the 32-byte merkle root
     * @throws StateSetException if any leaf is not exactly 32 bytes
     */
    fun merkleRoot(leaves: Array<ByteArray>): ByteArray = nativeMerkleRoot(leaves)

    @JvmStatic
    private external fun nativeJcsCanonicalize(json: String): ByteArray

    @JvmStatic
    private external fun nativePayloadPlainHash(json: String, salt: ByteArray?): ByteArray

    @JvmStatic
    private external fun nativeMerkleRoot(leaves: Array<ByteArray>): ByteArray
}
