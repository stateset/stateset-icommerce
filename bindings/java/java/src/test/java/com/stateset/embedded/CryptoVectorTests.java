package com.stateset.embedded;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Cross-binding compatibility test for the Java JNI binding.
 *
 * <p>Reads the language-neutral corpus at
 * {@code bindings/test-vectors/v1.json} and asserts the Java binding
 * produces byte-equal hex digests to Rust ground truth for every entry.
 * Counterparts: Rust ({@code crates/stateset-crypto/tests/cross_binding_vectors.rs}),
 * Node, Python, Go, and WASM.
 */
class CryptoVectorTests {

    /**
     * The corpus is at workspace-root {@code bindings/test-vectors/v1.json};
     * tests run from {@code bindings/java/java/}, so corpus is at
     * {@code ../../test-vectors/v1.json}.
     */
    private static final Path CORPUS_PATH =
        Paths.get("..", "..", "test-vectors", "v1.json");

    private static final Gson GSON = new Gson();

    private static JsonObject loadCorpus() throws IOException {
        String raw = new String(Files.readAllBytes(CORPUS_PATH), StandardCharsets.UTF_8);
        JsonObject parsed = JsonParser.parseString(raw).getAsJsonObject();
        assertEquals(1, parsed.get("version").getAsInt(), "corpus version must be 1");
        return parsed;
    }

    private static String hex(byte[] bytes) {
        StringBuilder sb = new StringBuilder(bytes.length * 2);
        for (byte b : bytes) {
            sb.append(String.format("%02x", b & 0xff));
        }
        return sb.toString();
    }

    private static byte[] fromHex(String s) {
        int len = s.length();
        byte[] out = new byte[len / 2];
        for (int i = 0; i < len; i += 2) {
            out[i / 2] = (byte) ((Character.digit(s.charAt(i), 16) << 4)
                + Character.digit(s.charAt(i + 1), 16));
        }
        return out;
    }

    @Test
    void corpusIsPresentAndVersionOne() throws IOException {
        JsonObject f = loadCorpus();
        JsonObject categories = f.getAsJsonObject("categories");
        assertTrue(categories.has("canonical_json"));
        assertTrue(categories.has("payload_plain_hash"));
        assertTrue(categories.has("merkle_root"));
    }

    @Test
    void canonicalJsonVectorsMatchGroundTruth() throws IOException, NoSuchAlgorithmException {
        JsonObject f = loadCorpus();
        JsonArray vectors = f.getAsJsonObject("categories").getAsJsonArray("canonical_json");
        for (JsonElement el : vectors) {
            JsonObject v = el.getAsJsonObject();
            String id = v.get("id").getAsString();
            String input = v.get("input").toString();
            String expected = v.get("expected_hex").getAsString();

            byte[] canonical = Crypto.jcsCanonicalize(input);
            byte[] digest = MessageDigest.getInstance("SHA-256").digest(canonical);
            assertEquals(expected, hex(digest),
                "canonical_json/" + id + ": SHA-256(jcs(input)) mismatch");
        }
    }

    @Test
    void payloadPlainHashVectorsMatchGroundTruth() throws IOException {
        JsonObject f = loadCorpus();
        JsonArray vectors = f.getAsJsonObject("categories").getAsJsonArray("payload_plain_hash");
        for (JsonElement el : vectors) {
            JsonObject v = el.getAsJsonObject();
            String id = v.get("id").getAsString();
            String input = v.get("input").toString();
            String expected = v.get("expected_hex").getAsString();
            byte[] salt = (v.has("salt_hex") && !v.get("salt_hex").isJsonNull())
                ? fromHex(v.get("salt_hex").getAsString())
                : null;

            byte[] digest = Crypto.payloadPlainHash(input, salt);
            assertEquals(expected, hex(digest),
                "payload_plain_hash/" + id + ": digest mismatch");
        }
    }

    @Test
    void merkleRootVectorsMatchGroundTruth() throws IOException {
        JsonObject f = loadCorpus();
        JsonArray vectors = f.getAsJsonObject("categories").getAsJsonArray("merkle_root");
        for (JsonElement el : vectors) {
            JsonObject v = el.getAsJsonObject();
            String id = v.get("id").getAsString();
            JsonArray leavesHex = v.getAsJsonArray("leaves_hex");
            String expected = v.get("expected_hex").getAsString();

            byte[][] leaves = new byte[leavesHex.size()][];
            for (int i = 0; i < leavesHex.size(); i++) {
                leaves[i] = fromHex(leavesHex.get(i).getAsString());
            }
            byte[] root = Crypto.merkleRoot(leaves);
            assertEquals(expected, hex(root),
                "merkle_root/" + id + ": root mismatch");
        }
    }
}
