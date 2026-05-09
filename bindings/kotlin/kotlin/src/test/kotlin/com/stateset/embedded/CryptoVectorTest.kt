package com.stateset.embedded

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.contentOrNull

import java.nio.file.Files
import java.nio.file.Paths
import java.security.MessageDigest

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/**
 * Cross-binding compatibility test for the Kotlin JNI binding.
 *
 * Reads the language-neutral corpus at `bindings/test-vectors/v1.json`
 * and asserts the Kotlin binding produces byte-equal hex digests to Rust
 * ground truth for every entry. Counterparts: Rust
 * (`crates/stateset-crypto/tests/cross_binding_vectors.rs`), Node, Python,
 * Go, WASM, and Java.
 */
class CryptoVectorTest {

    /**
     * The corpus is at workspace-root `bindings/test-vectors/v1.json`;
     * tests run from `bindings/kotlin/kotlin/`, so corpus is at
     * `../../test-vectors/v1.json`.
     */
    private val corpusPath = Paths.get("..", "..", "test-vectors", "v1.json")

    private fun loadCorpus(): JsonObject {
        val raw = String(Files.readAllBytes(corpusPath), Charsets.UTF_8)
        val parsed = Json.parseToJsonElement(raw).jsonObject
        assertEquals(1, parsed["version"]?.jsonPrimitive?.content?.toInt(),
            "corpus version must be 1")
        return parsed
    }

    private fun hex(bytes: ByteArray): String =
        bytes.joinToString("") { String.format("%02x", it.toInt() and 0xff) }

    private fun fromHex(s: String): ByteArray {
        require(s.length % 2 == 0) { "hex must have even length" }
        return ByteArray(s.length / 2) { i ->
            (Character.digit(s[i * 2], 16).shl(4) + Character.digit(s[i * 2 + 1], 16)).toByte()
        }
    }

    @Test
    fun corpusIsPresentAndVersionOne() {
        val corpus = loadCorpus()
        val cats = corpus["categories"]!!.jsonObject
        assertTrue(cats.containsKey("canonical_json"))
        assertTrue(cats.containsKey("payload_plain_hash"))
        assertTrue(cats.containsKey("merkle_root"))
    }

    @Test
    fun canonicalJsonVectorsMatchGroundTruth() {
        val corpus = loadCorpus()
        val vectors = corpus["categories"]!!.jsonObject["canonical_json"]!!.jsonArray
        val sha = MessageDigest.getInstance("SHA-256")
        for (el in vectors) {
            val v = el.jsonObject
            val id = v["id"]!!.jsonPrimitive.content
            val input = v["input"]!!.toString()
            val expected = v["expected_hex"]!!.jsonPrimitive.content

            val canonical = Crypto.jcsCanonicalize(input)
            val digest = sha.digest(canonical)
            assertEquals(expected, hex(digest),
                "canonical_json/$id: SHA-256(jcs(input)) mismatch")
            sha.reset()
        }
    }

    @Test
    fun payloadPlainHashVectorsMatchGroundTruth() {
        val corpus = loadCorpus()
        val vectors = corpus["categories"]!!.jsonObject["payload_plain_hash"]!!.jsonArray
        for (el in vectors) {
            val v = el.jsonObject
            val id = v["id"]!!.jsonPrimitive.content
            val input = v["input"]!!.toString()
            val expected = v["expected_hex"]!!.jsonPrimitive.content
            val saltHex = v["salt_hex"]?.jsonPrimitive?.contentOrNull
            val salt = saltHex?.let { fromHex(it) }

            val digest = Crypto.payloadPlainHash(input, salt)
            assertEquals(expected, hex(digest),
                "payload_plain_hash/$id: digest mismatch")
        }
    }

    @Test
    fun merkleRootVectorsMatchGroundTruth() {
        val corpus = loadCorpus()
        val vectors = corpus["categories"]!!.jsonObject["merkle_root"]!!.jsonArray
        for (el in vectors) {
            val v = el.jsonObject
            val id = v["id"]!!.jsonPrimitive.content
            val leavesHex = v["leaves_hex"]!!.jsonArray
            val expected = v["expected_hex"]!!.jsonPrimitive.content

            val leaves = Array(leavesHex.size) { i ->
                fromHex(leavesHex[i].jsonPrimitive.content)
            }
            val root = Crypto.merkleRoot(leaves)
            assertEquals(expected, hex(root),
                "merkle_root/$id: root mismatch")
        }
    }
}
