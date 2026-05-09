<?php

declare(strict_types=1);

use PHPUnit\Framework\TestCase;
use StateSet\Crypto;

/**
 * Cross-binding compatibility test for the PHP (ext-php-rs) binding.
 *
 * Reads the language-neutral corpus at `bindings/test-vectors/v1.json` and
 * asserts the PHP binding produces byte-equal hex digests to Rust ground
 * truth for every entry. Counterparts: Rust
 * (`crates/stateset-crypto/tests/cross_binding_vectors.rs`), Node, Python,
 * Go, WASM, Java, Kotlin, .NET, Swift, Ruby.
 *
 * Requires the native `stateset_embedded` extension to be loaded
 * (php -d extension=...). When the extension is not loaded, this test is
 * skipped — the autoloaded stub class would throw
 * `Error: cannot call abstract method`.
 */
class CryptoVectorTest extends TestCase
{
    /**
     * The corpus is at workspace-root `bindings/test-vectors/v1.json`;
     * phpunit runs from `bindings/php/`, so corpus is at
     * `../test-vectors/v1.json`.
     */
    private const CORPUS_PATH = __DIR__ . '/../../test-vectors/v1.json';

    protected function setUp(): void
    {
        if (!extension_loaded('stateset_embedded')) {
            $this->markTestSkipped(
                'stateset_embedded extension not loaded; run with ' .
                'php -d extension=$PWD/target/release/libstateset_embedded.so'
            );
        }
    }

    private function loadCorpus(): array
    {
        $raw = file_get_contents(self::CORPUS_PATH);
        $this->assertNotFalse($raw, 'corpus must be readable');
        $parsed = json_decode($raw, true, 512, JSON_THROW_ON_ERROR);
        $this->assertSame(1, $parsed['version'], 'corpus version must be 1');
        return $parsed;
    }

    public function testCorpusIsPresentAndVersionOne(): void
    {
        $corpus = $this->loadCorpus();
        $this->assertIsArray($corpus['categories']['canonical_json']);
        $this->assertIsArray($corpus['categories']['payload_plain_hash']);
        $this->assertIsArray($corpus['categories']['merkle_root']);
    }

    public function testCanonicalJsonVectorsMatchGroundTruth(): void
    {
        $corpus = $this->loadCorpus();
        foreach ($corpus['categories']['canonical_json'] as $v) {
            $input = json_encode($v['input'], JSON_THROW_ON_ERROR);
            $canonical = Crypto::jcsCanonicalize($input);
            $digest = hash('sha256', $canonical);
            $this->assertSame(
                $v['expected_hex'],
                $digest,
                "canonical_json/{$v['id']}: SHA-256(jcs(input)) mismatch",
            );
        }
    }

    public function testPayloadPlainHashVectorsMatchGroundTruth(): void
    {
        $corpus = $this->loadCorpus();
        foreach ($corpus['categories']['payload_plain_hash'] as $v) {
            $input = json_encode($v['input'], JSON_THROW_ON_ERROR);
            $salt = isset($v['salt_hex']) ? hex2bin($v['salt_hex']) : null;
            $digest = Crypto::payloadPlainHash($input, $salt);
            $this->assertSame(
                $v['expected_hex'],
                bin2hex($digest),
                "payload_plain_hash/{$v['id']}: digest mismatch",
            );
        }
    }

    public function testMerkleRootVectorsMatchGroundTruth(): void
    {
        $corpus = $this->loadCorpus();
        foreach ($corpus['categories']['merkle_root'] as $v) {
            $leaves = array_map('hex2bin', $v['leaves_hex']);
            $root = Crypto::merkleRoot($leaves);
            $this->assertSame(
                $v['expected_hex'],
                bin2hex($root),
                "merkle_root/{$v['id']}: root mismatch",
            );
        }
    }
}
