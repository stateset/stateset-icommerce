#ifndef STATESET_H
#define STATESET_H

/*
 * C declarations for the StateSet native library (libstateset_swift), a thin
 * re-export of the stateset-ffi crate's C ABI:
 *
 *   crates/stateset-ffi/src/json_api.rs   -- open / call / destroy
 *   crates/stateset-ffi/src/strings.rs    -- stateset_string_free
 *   crates/stateset-ffi/src/error.rs      -- stateset_last_error_message
 *   crates/stateset-ffi/src/version.rs    -- stateset_abi_version
 *   crates/stateset-ffi/src/crypto_api.rs -- stateset_crypto_*
 *
 * Signatures must stay byte-for-byte in sync with the Rust side;
 * crates/stateset-ffi's `swift_header_declares_every_json_export` test checks
 * the names.
 */

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Opaque engine handle. */
typedef void *StateSetHandle;

/* ABI major version of stateset-ffi (the Swift binding expects 1). */
uint32_t stateset_abi_version(void);

/*
 * Open (or create) a store at `db_path` (":memory:" for an ephemeral one) and
 * write its handle to `*out_handle`. Returns 0 on success, otherwise an
 * FfiErrorCode (1 not found, 2 invalid argument, 3 internal, 4 database,
 * 5 serialization, 6 null pointer, 7 UTF-8); the message is available from
 * stateset_last_error_message().
 */
int32_t stateset_json_open(const char *db_path, StateSetHandle *out_handle);

/*
 * Invoke `method` ("orders.create", ...) with `args_json` (a JSON object, or
 * NULL) and return a JSON envelope:
 *   {"ok":true,"result":...}
 *   {"ok":false,"error":{"code":N,"kind":"not_found","message":"..."}}
 * The returned string is owned by the caller: release it with
 * stateset_string_free(). Money crosses as exact decimal strings.
 */
char *stateset_json_call(StateSetHandle handle, const char *method, const char *args_json);

/* Close a store; waits for in-flight calls. NULL is a no-op. */
void stateset_destroy(StateSetHandle handle);

/* Free a string returned by stateset_json_call. NULL is a no-op. */
void stateset_string_free(char *s);

/* Last error on this thread, or NULL. Borrowed: do NOT free. */
const char *stateset_last_error_message(void);

/*
 * Cross-binding crypto primitives (bindings/test-vectors/v1.json).
 * Return 0 on success; -1 null/invalid input; -2 computation error;
 * -3 panic caught at the boundary.
 */
int stateset_crypto_jcs_canonicalize(const char *json_in, uint8_t **out_ptr, size_t *out_len);
void stateset_crypto_free_buffer(uint8_t *ptr, size_t len);
int stateset_crypto_payload_plain_hash(const char *json_in, const uint8_t *salt_in, size_t salt_len,
                                       uint8_t *out_buf32);
int stateset_crypto_merkle_root(const uint8_t *leaves_in, size_t leaf_count, uint8_t *out_buf32);

#ifdef __cplusplus
}
#endif

#endif /* STATESET_H */
