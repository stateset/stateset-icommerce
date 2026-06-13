#ifndef STATESET_H
#define STATESET_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef void *StateSetHandle;

StateSetHandle stateset_commerce_new(const char *db_path);
void stateset_commerce_free(StateSetHandle handle);
void stateset_string_free(char *s);
char *stateset_get_last_error(void);

/*
 * Cross-binding crypto primitives.
 *
 * Declarations for the C-FFI exports defined in `bindings/swift/src/lib.rs`,
 * which delegate to the `stateset-crypto` Rust crate. Signatures must stay
 * byte-for-byte in sync with the Rust side.
 */

/*
 * JCS-canonicalize (RFC 8785) a NUL-terminated JSON string. On success
 * writes a heap buffer pointer to `out_ptr` and its length to `out_len`;
 * release the buffer with `stateset_crypto_free_buffer`. Returns 0 on
 * success; -1 on null/invalid input; -2 on canonicalization error.
 */
int stateset_crypto_jcs_canonicalize(const char *json_in,
                                     uint8_t **out_ptr,
                                     size_t *out_len);

/*
 * Free a buffer returned by `stateset_crypto_jcs_canonicalize`.
 * `ptr` must come from `stateset_crypto_jcs_canonicalize`; `len` must match.
 */
void stateset_crypto_free_buffer(uint8_t *ptr, size_t len);

/*
 * Compute the VES v1.0 payload-plain hash for a JSON payload. Writes 32
 * bytes into `out_buf32`. `salt_in` may be NULL; if non-NULL, `salt_len`
 * must be 16. Returns 0 on success; -1/-2 on errors.
 */
int stateset_crypto_payload_plain_hash(const char *json_in,
                                       const uint8_t *salt_in,
                                       size_t salt_len,
                                       uint8_t *out_buf32);

/*
 * Compute the merkle root of `leaf_count` 32-byte leaves stored
 * contiguously in `leaves_in` (may be NULL when `leaf_count == 0`, which
 * yields the empty-tree sentinel). Writes 32 bytes into `out_buf32`.
 * Returns 0 on success; -1 on invalid input.
 */
int stateset_crypto_merkle_root(const uint8_t *leaves_in,
                                size_t leaf_count,
                                uint8_t *out_buf32);

#ifdef __cplusplus
}
#endif

#endif
