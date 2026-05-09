// Cross-binding crypto primitives.
//
// Thin wrappers over the C-FFI exposed by `bindings/go/src/lib.rs` so the Go
// binding can verify the language-neutral test corpus at
// `bindings/test-vectors/v1.json`. The Rust ground truth lives at
// `crates/stateset-crypto/tests/cross_binding_vectors.rs` and identical
// verifiers exist for Node and Python.

package stateset

/*
#cgo LDFLAGS: -L${SRCDIR}/../../../target/release -lstateset_go -lm -ldl -lpthread
#cgo linux LDFLAGS: -Wl,-rpath,${SRCDIR}/../../../target/release
#cgo darwin LDFLAGS: -Wl,-rpath,${SRCDIR}/../../../target/release

#include <stdlib.h>
#include <stdint.h>

extern void stateset_crypto_free_buffer(uint8_t* ptr, size_t len);
extern int stateset_crypto_jcs_canonicalize(const char* json_in, uint8_t** out_ptr, size_t* out_len);
extern int stateset_crypto_payload_plain_hash(const char* json_in, const uint8_t* salt_in, size_t salt_len, uint8_t* out_buf32);
extern int stateset_crypto_merkle_root(const uint8_t* leaves_in, size_t leaf_count, uint8_t* out_buf32);
*/
import "C"

import (
	"errors"
	"unsafe"
)

// JCSCanonicalize returns the RFC 8785 JCS canonical bytes of jsonStr.
//
// The returned slice is a Go copy; the underlying C buffer is freed before
// return.
func JCSCanonicalize(jsonStr string) ([]byte, error) {
	cStr := C.CString(jsonStr)
	defer C.free(unsafe.Pointer(cStr))

	var ptr *C.uint8_t
	var length C.size_t
	rc := C.stateset_crypto_jcs_canonicalize(cStr, &ptr, &length)
	if rc != 0 {
		return nil, errors.New("jcs_canonicalize failed (invalid JSON or canonicalization error)")
	}
	defer C.stateset_crypto_free_buffer(ptr, length)

	out := C.GoBytes(unsafe.Pointer(ptr), C.int(length))
	return out, nil
}

// PayloadPlainHash returns the 32-byte VES v1.0 payload-plain hash of the
// given JSON payload. salt may be nil; if non-nil it must be exactly 16 bytes.
func PayloadPlainHash(jsonStr string, salt []byte) ([]byte, error) {
	cStr := C.CString(jsonStr)
	defer C.free(unsafe.Pointer(cStr))

	out := make([]byte, 32)
	var saltPtr *C.uint8_t
	var saltLen C.size_t
	if salt != nil {
		if len(salt) != 16 {
			return nil, errors.New("salt must be exactly 16 bytes")
		}
		saltPtr = (*C.uint8_t)(unsafe.Pointer(&salt[0]))
		saltLen = C.size_t(len(salt))
	}

	rc := C.stateset_crypto_payload_plain_hash(
		cStr,
		saltPtr,
		saltLen,
		(*C.uint8_t)(unsafe.Pointer(&out[0])),
	)
	if rc != 0 {
		return nil, errors.New("payload_plain_hash failed (invalid JSON or hash error)")
	}
	return out, nil
}

// MerkleRoot returns the 32-byte merkle root of a list of 32-byte leaves.
// An empty list returns the empty-tree sentinel.
func MerkleRoot(leaves [][]byte) ([]byte, error) {
	out := make([]byte, 32)
	if len(leaves) == 0 {
		rc := C.stateset_crypto_merkle_root(nil, 0, (*C.uint8_t)(unsafe.Pointer(&out[0])))
		if rc != 0 {
			return nil, errors.New("merkle_root failed for empty leaves")
		}
		return out, nil
	}
	flat := make([]byte, 0, len(leaves)*32)
	for i, leaf := range leaves {
		if len(leaf) != 32 {
			return nil, errors.New("merkle_root: every leaf must be 32 bytes")
		}
		flat = append(flat, leaf...)
		_ = i
	}
	rc := C.stateset_crypto_merkle_root(
		(*C.uint8_t)(unsafe.Pointer(&flat[0])),
		C.size_t(len(leaves)),
		(*C.uint8_t)(unsafe.Pointer(&out[0])),
	)
	if rc != 0 {
		return nil, errors.New("merkle_root failed")
	}
	return out, nil
}
