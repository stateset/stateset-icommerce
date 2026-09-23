/-!
# Binding of the command attestation Merkle path

`stateset-sync/src/attestation.rs` starts with a leaf hash and combines one
sibling per level. The low bit of the current index selects left or right,
then the index is divided by two. `Digest.node` is a free constructor here:
the theorem is conditional on the real SHA-256 node hash having no collision
in the paths under consideration. It is not a proof of SHA-256 itself.

The verifier also requires exactly `ceil(log₂ total_leaves)` siblings. That
shape rule is essential: without it, a leaf that is itself an internal node
could be accepted as the root with an empty path and a false leaf count.
-/

namespace MerklePath

inductive Digest where
  | atom : Nat → Digest
  | node : Digest → Digest → Digest
  deriving DecidableEq, Repr

/-- The same bottom-up path operation as `verify_merkle_path` in Rust. -/
def commit (leaf : Digest) (index : Nat) : List Digest → Digest
  | [] => leaf
  | sibling :: rest =>
      let parent := if index % 2 = 0 then .node leaf sibling else .node sibling leaf
      commit parent (index / 2) rest

/-- The index remaining after consuming a path. -/
def finalIndex (index : Nat) : List Digest → Nat
  | [] => index
  | _ :: rest => finalIndex (index / 2) rest

/-- A path wide enough for its index consumes every index bit. -/
theorem index_exhausted (index : Nat) (path : List Digest)
    (hindex : index < 2 ^ path.length) : finalIndex index path = 0 := by
  induction path generalizing index with
  | nil =>
      simp only [List.length_nil, Nat.pow_zero] at hindex
      simp [finalIndex]
      omega
  | cons _ rest ih =>
      have hsmall : index / 2 < 2 ^ rest.length := by
        simp only [List.length_cons, Nat.pow_succ] at hindex
        omega
      exact ih (index / 2) hsmall

/-- Equal canonical-length paths at the same index and root bind the leaf.
This uses the idealized, collision-free `node` constructor. -/
theorem leaf_binding (index : Nat) (leaf₁ leaf₂ : Digest)
    (path₁ path₂ : List Digest)
    (hlen : path₁.length = path₂.length)
    (hroot : commit leaf₁ index path₁ = commit leaf₂ index path₂) :
    leaf₁ = leaf₂ := by
  induction path₁ generalizing index leaf₁ leaf₂ path₂ with
  | nil =>
      cases path₂ with
      | nil => simpa [commit] using hroot
      | cons _ _ => simp at hlen
  | cons sibling₁ rest₁ ih =>
      cases path₂ with
      | nil => simp at hlen
      | cons sibling₂ rest₂ =>
          have htail : rest₁.length = rest₂.length := by simpa using hlen
          simp only [commit] at hroot
          have hparent := ih (index / 2)
            (if index % 2 = 0 then .node leaf₁ sibling₁ else .node sibling₁ leaf₁)
            (if index % 2 = 0 then .node leaf₂ sibling₂ else .node sibling₂ leaf₂)
            rest₂ htail hroot
          split at hparent <;> cases hparent <;> rfl

/-- A malformed empty path can make a node look like a leaf. The depth check
excludes this example when the claimed tree has two leaves. -/
example (a b : Digest) : commit (.node a b) 0 [] = commit a 0 [b] := by
  rfl

end MerklePath
