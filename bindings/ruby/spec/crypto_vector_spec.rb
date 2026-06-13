# frozen_string_literal: true

# Cross-binding compatibility test for the Ruby (magnus) binding.
#
# Reads the language-neutral corpus at `bindings/test-vectors/v1.json` and
# asserts the Ruby binding produces byte-equal hex digests to Rust ground
# truth for every entry. Counterparts: Rust
# (`crates/stateset-crypto/tests/cross_binding_vectors.rs`), Node, Python,
# Go, WASM, Java, Kotlin, .NET, Swift.

require 'spec_helper'
require 'json'
require 'digest'

RSpec.describe StateSet::Crypto do
  # The corpus is at workspace-root `bindings/test-vectors/v1.json`;
  # rspec runs from `bindings/ruby/`, so corpus is at
  # `../test-vectors/v1.json`.
  CORPUS_PATH = File.expand_path('../../test-vectors/v1.json', __dir__)

  let(:corpus) do
    raw = File.read(CORPUS_PATH)
    parsed = JSON.parse(raw)
    expect(parsed['version']).to eq(1)
    parsed
  end

  it 'has the corpus available and at version 1' do
    expect(corpus['categories']).to be_a(Hash)
    expect(corpus['categories']['canonical_json']).to be_an(Array)
    expect(corpus['categories']['payload_plain_hash']).to be_an(Array)
    expect(corpus['categories']['merkle_root']).to be_an(Array)
  end

  it 'matches Rust ground truth for every canonical_json vector' do
    corpus['categories']['canonical_json'].each do |v|
      input = v['input'].to_json
      canonical = StateSet::Crypto.jcs_canonicalize(input)
      digest = Digest::SHA256.hexdigest(canonical)
      expect(digest).to eq(v['expected_hex']),
        "canonical_json/#{v['id']}: SHA-256(jcs(input)) mismatch"
    end
  end

  it 'matches Rust ground truth for every payload_plain_hash vector' do
    corpus['categories']['payload_plain_hash'].each do |v|
      input = v['input'].to_json
      salt = v['salt_hex'] ? [v['salt_hex']].pack('H*') : nil
      digest = StateSet::Crypto.payload_plain_hash(input, salt)
      expect(digest.unpack1('H*')).to eq(v['expected_hex']),
        "payload_plain_hash/#{v['id']}: digest mismatch"
    end
  end

  it 'matches Rust ground truth for every merkle_root vector' do
    corpus['categories']['merkle_root'].each do |v|
      leaves = v['leaves_hex'].map { |h| [h].pack('H*') }
      root = StateSet::Crypto.merkle_root(leaves)
      expect(root.unpack1('H*')).to eq(v['expected_hex']),
        "merkle_root/#{v['id']}: root mismatch"
    end
  end
end
