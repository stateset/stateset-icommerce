import CryptoKit
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
            let digest = SHA256.hash(data: canonical)
            let actual = digest.map { String(format: "%02x", $0) }.joined()
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
}
