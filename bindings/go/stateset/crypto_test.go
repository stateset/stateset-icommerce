// Cross-binding compatibility test for the Go binding.
//
// Reads the language-neutral corpus at `bindings/test-vectors/v1.json` and
// asserts the Go binding produces byte-equal hex digests to Rust ground truth
// for every entry. Counterparts:
//   - Rust:   crates/stateset-crypto/tests/cross_binding_vectors.rs
//   - Node:   bindings/node/test/cross-binding-vectors.js
//   - Python: bindings/python/tests/test_cross_binding_vectors.py

package stateset

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

type canonicalJSONVector struct {
	ID          string          `json:"id"`
	Input       json.RawMessage `json:"input"`
	ExpectedHex string          `json:"expected_hex"`
}

type payloadHashVector struct {
	ID          string          `json:"id"`
	Input       json.RawMessage `json:"input"`
	SaltHex     string          `json:"salt_hex,omitempty"`
	ExpectedHex string          `json:"expected_hex"`
}

type merkleVector struct {
	ID          string   `json:"id"`
	LeavesHex   []string `json:"leaves_hex"`
	ExpectedHex string   `json:"expected_hex"`
}

type vectorFile struct {
	Version    int `json:"version"`
	Categories struct {
		CanonicalJSON    []canonicalJSONVector `json:"canonical_json"`
		PayloadPlainHash []payloadHashVector   `json:"payload_plain_hash"`
		MerkleRoot       []merkleVector        `json:"merkle_root"`
	} `json:"categories"`
}

func loadCorpus(t *testing.T) vectorFile {
	t.Helper()
	// Path is workspace-root `bindings/test-vectors/v1.json`. Tests run from
	// `bindings/go/stateset/`, so corpus is at `../../test-vectors/v1.json`.
	path := filepath.Join("..", "..", "test-vectors", "v1.json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read corpus %s: %v", path, err)
	}
	var f vectorFile
	if err := json.Unmarshal(data, &f); err != nil {
		t.Fatalf("parse corpus: %v", err)
	}
	if f.Version != 1 {
		t.Fatalf("expected corpus version 1, got %d", f.Version)
	}
	return f
}

func TestCorpusPresentAndVersioned(t *testing.T) {
	f := loadCorpus(t)
	if len(f.Categories.CanonicalJSON) == 0 {
		t.Error("canonical_json category is empty")
	}
	if len(f.Categories.PayloadPlainHash) == 0 {
		t.Error("payload_plain_hash category is empty")
	}
	if len(f.Categories.MerkleRoot) == 0 {
		t.Error("merkle_root category is empty")
	}
}

func TestCanonicalJSONVectorsMatchGroundTruth(t *testing.T) {
	f := loadCorpus(t)
	for _, v := range f.Categories.CanonicalJSON {
		canonical, err := JCSCanonicalize(string(v.Input))
		if err != nil {
			t.Errorf("canonical_json/%s: JCSCanonicalize: %v", v.ID, err)
			continue
		}
		digest := sha256.Sum256(canonical)
		actual := hex.EncodeToString(digest[:])
		if actual != v.ExpectedHex {
			t.Errorf("canonical_json/%s: expected %s, got %s", v.ID, v.ExpectedHex, actual)
		}
	}
}

func TestPayloadPlainHashVectorsMatchGroundTruth(t *testing.T) {
	f := loadCorpus(t)
	for _, v := range f.Categories.PayloadPlainHash {
		var salt []byte
		if v.SaltHex != "" {
			s, err := hex.DecodeString(v.SaltHex)
			if err != nil {
				t.Errorf("payload_plain_hash/%s: decode salt: %v", v.ID, err)
				continue
			}
			salt = s
		}
		digest, err := PayloadPlainHash(string(v.Input), salt)
		if err != nil {
			t.Errorf("payload_plain_hash/%s: PayloadPlainHash: %v", v.ID, err)
			continue
		}
		actual := hex.EncodeToString(digest)
		if actual != v.ExpectedHex {
			t.Errorf("payload_plain_hash/%s: expected %s, got %s", v.ID, v.ExpectedHex, actual)
		}
	}
}

func TestMerkleRootVectorsMatchGroundTruth(t *testing.T) {
	f := loadCorpus(t)
	for _, v := range f.Categories.MerkleRoot {
		leaves := make([][]byte, len(v.LeavesHex))
		for i, h := range v.LeavesHex {
			b, err := hex.DecodeString(h)
			if err != nil {
				t.Errorf("merkle_root/%s: decode leaf %d: %v", v.ID, i, err)
				continue
			}
			leaves[i] = b
		}
		root, err := MerkleRoot(leaves)
		if err != nil {
			t.Errorf("merkle_root/%s: MerkleRoot: %v", v.ID, err)
			continue
		}
		actual := hex.EncodeToString(root)
		if actual != v.ExpectedHex {
			t.Errorf("merkle_root/%s: expected %s, got %s", v.ID, v.ExpectedHex, actual)
		}
	}
}
