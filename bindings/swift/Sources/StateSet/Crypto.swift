import Foundation
import StateSetC

/// Cross-binding cryptographic primitives.
///
/// Thin Swift wrappers over the C-FFI exports in `bindings/swift/src/lib.rs`
/// that delegate to the `stateset-crypto` Rust crate. The same primitives
/// are exported from every StateSet binding and verified against the
/// language-neutral test corpus at `bindings/test-vectors/v1.json`.
///
/// All methods are static and thread-safe. On invalid input or runtime
/// failure they throw `Crypto.Error`.
public enum Crypto {

    public enum Error: Swift.Error, CustomStringConvertible {
        case invalidInput(String)
        case operationFailed(String)

        public var description: String {
            switch self {
            case .invalidInput(let msg): return "invalid input: \(msg)"
            case .operationFailed(let msg): return "operation failed: \(msg)"
            }
        }
    }

    /// RFC 8785 JCS canonical-form bytes for a JSON string.
    public static func jcsCanonicalize(_ json: String) throws -> Data {
        var outPtr: UnsafeMutablePointer<UInt8>? = nil
        var outLen: size_t = 0
        let rc = json.withCString { cstr in
            stateset_crypto_jcs_canonicalize(cstr, &outPtr, &outLen)
        }
        if rc != 0 {
            throw Error.operationFailed("jcs_canonicalize rc=\(rc)")
        }
        guard let ptr = outPtr else {
            throw Error.operationFailed("jcs_canonicalize returned null pointer")
        }
        defer { stateset_crypto_free_buffer(ptr, outLen) }
        return Data(bytes: ptr, count: outLen)
    }

    /// VES v1.0 payload-plain hash. Returns 32 bytes.
    /// `salt`, when non-nil, must be exactly 16 bytes.
    public static func payloadPlainHash(_ json: String, salt: Data? = nil) throws -> Data {
        if let s = salt, s.count != 16 {
            throw Error.invalidInput("salt must be exactly 16 bytes, got \(s.count)")
        }
        var output = [UInt8](repeating: 0, count: 32)
        let rc: Int32 = json.withCString { cstr in
            output.withUnsafeMutableBufferPointer { outBuf in
                if let s = salt {
                    return s.withUnsafeBytes { saltRaw -> Int32 in
                        let saltPtr = saltRaw.bindMemory(to: UInt8.self).baseAddress
                        return stateset_crypto_payload_plain_hash(
                            cstr,
                            saltPtr,
                            s.count,
                            outBuf.baseAddress)
                    }
                } else {
                    return stateset_crypto_payload_plain_hash(
                        cstr,
                        nil,
                        0,
                        outBuf.baseAddress)
                }
            }
        }
        if rc != 0 {
            throw Error.operationFailed("payload_plain_hash rc=\(rc)")
        }
        return Data(output)
    }

    /// Merkle root of a list of 32-byte leaves. Returns 32 bytes.
    /// An empty list yields the empty-tree sentinel from `stateset-crypto`.
    public static func merkleRoot(_ leaves: [Data]) throws -> Data {
        var output = [UInt8](repeating: 0, count: 32)
        if leaves.isEmpty {
            let rc = output.withUnsafeMutableBufferPointer { outBuf in
                stateset_crypto_merkle_root(nil, 0, outBuf.baseAddress)
            }
            if rc != 0 {
                throw Error.operationFailed("merkle_root rc=\(rc) for empty leaves")
            }
            return Data(output)
        }
        var flat = [UInt8]()
        flat.reserveCapacity(leaves.count * 32)
        for (i, leaf) in leaves.enumerated() {
            if leaf.count != 32 {
                throw Error.invalidInput("leaf \(i) must be 32 bytes, got \(leaf.count)")
            }
            flat.append(contentsOf: leaf)
        }
        let rc: Int32 = flat.withUnsafeBufferPointer { flatBuf in
            output.withUnsafeMutableBufferPointer { outBuf in
                stateset_crypto_merkle_root(
                    flatBuf.baseAddress,
                    leaves.count,
                    outBuf.baseAddress)
            }
        }
        if rc != 0 {
            throw Error.operationFailed("merkle_root rc=\(rc)")
        }
        return Data(output)
    }
}
