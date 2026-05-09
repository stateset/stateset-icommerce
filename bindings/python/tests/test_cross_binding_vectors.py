"""Cross-binding compatibility test for the Python binding.

Reads the language-neutral corpus at ``bindings/test-vectors/v1.json`` and
asserts the Python binding produces byte-equal hex digests to Rust ground
truth for every entry. Counterparts:
- Rust: ``crates/stateset-crypto/tests/cross_binding_vectors.rs``
- Node: ``bindings/node/test/cross-binding-vectors.js``
"""

import hashlib
import json
import os
from pathlib import Path

import pytest

from stateset_embedded import jcs_canonicalize, merkle_root, payload_plain_hash

CORPUS_PATH = (
    Path(__file__).resolve().parent.parent.parent / "test-vectors" / "v1.json"
)


def _load_corpus():
    raw = CORPUS_PATH.read_text(encoding="utf-8")
    parsed = json.loads(raw)
    assert parsed["version"] == 1, "corpus version must be 1"
    return parsed


def test_corpus_present_and_versioned():
    corpus = _load_corpus()
    assert "categories" in corpus
    assert isinstance(corpus["categories"]["canonical_json"], list)
    assert isinstance(corpus["categories"]["payload_plain_hash"], list)
    assert isinstance(corpus["categories"]["merkle_root"], list)


def test_canonical_json_vectors_match_ground_truth():
    """Every canonical_json vector hashes to the Rust expected digest."""
    corpus = _load_corpus()
    for vec in corpus["categories"]["canonical_json"]:
        canonical = bytes(jcs_canonicalize(json.dumps(vec["input"])))
        digest = hashlib.sha256(canonical).hexdigest()
        assert digest == vec["expected_hex"], (
            f"canonical_json/{vec['id']}: expected {vec['expected_hex']}, "
            f"got {digest}"
        )


def test_payload_plain_hash_vectors_match_ground_truth():
    """Every payload_plain_hash vector matches the Rust expected digest."""
    corpus = _load_corpus()
    for vec in corpus["categories"]["payload_plain_hash"]:
        salt = bytes.fromhex(vec["salt_hex"]) if vec.get("salt_hex") else None
        digest = bytes(payload_plain_hash(json.dumps(vec["input"]), salt)).hex()
        assert digest == vec["expected_hex"], (
            f"payload_plain_hash/{vec['id']}: expected {vec['expected_hex']}, "
            f"got {digest}"
        )


def test_merkle_root_vectors_match_ground_truth():
    """Every merkle_root vector matches the Rust expected digest."""
    corpus = _load_corpus()
    for vec in corpus["categories"]["merkle_root"]:
        leaves = [bytes.fromhex(h) for h in vec["leaves_hex"]]
        digest = bytes(merkle_root(leaves)).hex()
        assert digest == vec["expected_hex"], (
            f"merkle_root/{vec['id']}: expected {vec['expected_hex']}, "
            f"got {digest}"
        )
