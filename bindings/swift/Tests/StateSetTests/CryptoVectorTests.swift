#if canImport(CryptoKit)
import CryptoKit
#endif
import Foundation
import XCTest
@testable import StateSet

/// Cross-binding compatibility test for the Swift C-FFI binding.
///
/// Reads the language-neutral corpus at `bindings/test-vectors/v1.json` and
/// asserts the Swift binding produces byte-equal hex digests to Rust ground
/// truth for every entry. Counterparts: Rust
/// (`crates/stateset-crypto/tests/cross_binding_vectors.rs`), Node, Python,
/// Go, WASM, Java, Kotlin, and .NET.
final class CryptoVectorTests: XCTestCase {

    /// Walk up from the test working directory until we find
    /// `bindings/test-vectors/v1.json`. Swift Package Manager's test runner
    /// usually sets the cwd to the package root (`bindings/swift`), but be
    /// defensive in case it changes.
    private static func findCorpus() -> String {
        let fm = FileManager.default
        var dir = URL(fileURLWithPath: fm.currentDirectoryPath)
        while true {
            let candidate = dir.appendingPathComponent("bindings/test-vectors/v1.json")
            if fm.fileExists(atPath: candidate.path) {
                return candidate.path
            }
            let parent = dir.deletingLastPathComponent()
            if parent.path == dir.path { break }
            dir = parent
        }
        XCTFail("could not locate bindings/test-vectors/v1.json from \(fm.currentDirectoryPath)")
        return ""
    }

    private static func loadCorpus() throws -> [String: Any] {
        let path = findCorpus()
        let data = try Data(contentsOf: URL(fileURLWithPath: path))
        let json = try JSONSerialization.jsonObject(with: data, options: [])
        guard let dict = json as? [String: Any] else {
            XCTFail("corpus root is not a JSON object")
            return [:]
        }
        XCTAssertEqual(dict["version"] as? Int, 1, "corpus version must be 1")
        return dict
    }

    private static func toJSONString(_ obj: Any) throws -> String {
        // JSCanonicalization doesn't care about input formatting; this just
        // re-serializes the parsed JSON value into a string we can hand to
        // the binding. JSONSerialization is happy to round-trip primitives,
        // arrays, and dictionaries.
        let data = try JSONSerialization.data(
            withJSONObject: obj,
            options: [.fragmentsAllowed])
        return String(data: data, encoding: .utf8)!
    }

    private static func hex(_ data: Data) -> String {
        return data.map { String(format: "%02x", $0) }.joined()
    }

    private static func fromHex(_ s: String) -> Data {
        var out = Data(capacity: s.count / 2)
        var idx = s.startIndex
        while idx < s.endIndex {
            let next = s.index(idx, offsetBy: 2)
            if let byte = UInt8(s[idx..<next], radix: 16) {
                out.append(byte)
            }
            idx = next
        }
        return out
    }

    func testCorpusIsPresentAndVersionOne() throws {
        let corpus = try Self.loadCorpus()
        guard let cats = corpus["categories"] as? [String: Any] else {
            XCTFail("missing categories"); return
        }
        XCTAssertNotNil(cats["canonical_json"])
        XCTAssertNotNil(cats["payload_plain_hash"])
        XCTAssertNotNil(cats["merkle_root"])
    }

    func testCanonicalJSONVectorsMatchGroundTruth() throws {
        let corpus = try Self.loadCorpus()
        let cats = corpus["categories"] as! [String: Any]
        let vectors = cats["canonical_json"] as! [[String: Any]]
        for v in vectors {
            let id = v["id"] as! String
            let input = v["input"]!
            let expected = v["expected_hex"] as! String

            let inputStr = try Self.toJSONString(input)
            let canonical = try Crypto.jcsCanonicalize(inputStr)
            let actual = Self.hex(Self.sha256(canonical))
            XCTAssertEqual(actual, expected,
                "canonical_json/\(id): SHA-256(jcs(input)) mismatch")
        }
    }

    func testPayloadPlainHashVectorsMatchGroundTruth() throws {
        let corpus = try Self.loadCorpus()
        let cats = corpus["categories"] as! [String: Any]
        let vectors = cats["payload_plain_hash"] as! [[String: Any]]
        for v in vectors {
            let id = v["id"] as! String
            let input = v["input"]!
            let expected = v["expected_hex"] as! String
            let salt = (v["salt_hex"] as? String).map { Self.fromHex($0) }

            let inputStr = try Self.toJSONString(input)
            let digest = try Crypto.payloadPlainHash(inputStr, salt: salt)
            XCTAssertEqual(Self.hex(digest), expected,
                "payload_plain_hash/\(id): digest mismatch")
        }
    }

    func testMerkleRootVectorsMatchGroundTruth() throws {
        let corpus = try Self.loadCorpus()
        let cats = corpus["categories"] as! [String: Any]
        let vectors = cats["merkle_root"] as! [[String: Any]]
        for v in vectors {
            let id = v["id"] as! String
            let leavesHex = v["leaves_hex"] as! [String]
            let expected = v["expected_hex"] as! String

            let leaves = leavesHex.map { Self.fromHex($0) }
            let root = try Crypto.merkleRoot(leaves)
            XCTAssertEqual(Self.hex(root), expected,
                "merkle_root/\(id): root mismatch")
        }
    }

    /// SHA-256 of `data`. CryptoKit on Apple platforms; a small portable
    /// implementation elsewhere (CryptoKit does not exist on Linux).
    static func sha256(_ data: Data) -> Data {
        #if canImport(CryptoKit)
        return Data(SHA256.hash(data: data))
        #else
        return PortableSHA256.hash(data)
        #endif
    }
}

/// FIPS 180-4 SHA-256, used only by the tests on platforms without CryptoKit.
enum PortableSHA256 {
    private static let k: [UInt32] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ]

    private static func rotr(_ x: UInt32, _ n: UInt32) -> UInt32 { (x >> n) | (x << (32 - n)) }

    static func hash(_ data: Data) -> Data {
        var h: [UInt32] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19]
        var msg = [UInt8](data)
        let bitLen = UInt64(msg.count) * 8
        msg.append(0x80)
        while msg.count % 64 != 56 { msg.append(0) }
        for i in (0..<8).reversed() { msg.append(UInt8((bitLen >> (UInt64(i) * 8)) & 0xff)) }
        var w = [UInt32](repeating: 0, count: 64)
        for chunk in stride(from: 0, to: msg.count, by: 64) {
            for i in 0..<16 {
                let b = chunk + i * 4
                w[i] = UInt32(msg[b]) << 24 | UInt32(msg[b + 1]) << 16 | UInt32(msg[b + 2]) << 8 | UInt32(msg[b + 3])
            }
            for i in 16..<64 {
                let s0 = rotr(w[i - 15], 7) ^ rotr(w[i - 15], 18) ^ (w[i - 15] >> 3)
                let s1 = rotr(w[i - 2], 17) ^ rotr(w[i - 2], 19) ^ (w[i - 2] >> 10)
                w[i] = w[i - 16] &+ s0 &+ w[i - 7] &+ s1
            }
            var (a, b, c, d, e, f, g, hh) = (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7])
            for i in 0..<64 {
                let s1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25)
                let ch = (e & f) ^ (~e & g)
                let t1 = hh &+ s1 &+ ch &+ k[i] &+ w[i]
                let s0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22)
                let maj = (a & b) ^ (a & c) ^ (b & c)
                let t2 = s0 &+ maj
                hh = g; g = f; f = e; e = d &+ t1; d = c; c = b; b = a; a = t1 &+ t2
            }
            h[0] = h[0] &+ a; h[1] = h[1] &+ b; h[2] = h[2] &+ c; h[3] = h[3] &+ d
            h[4] = h[4] &+ e; h[5] = h[5] &+ f; h[6] = h[6] &+ g; h[7] = h[7] &+ hh
        }
        var out = Data()
        for v in h { for s in stride(from: 24, through: 0, by: -8) { out.append(UInt8((v >> UInt32(s)) & 0xff)) } }
        return out
    }
}
