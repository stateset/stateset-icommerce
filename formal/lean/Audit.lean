import Lean

/-! Audit kernel-safe declarations, including private/unused declarations and helpers.
No source-text theorem list or parsing of human-readable `#print` output is used. -/
open Lean Elab Command

elab "audit_module " module:ident : command => do
  let env ← getEnv
  let some index := env.getModuleIdx? module.getId
    | throwError "audit module was not imported: {module.getId}"
  let mut declarations := 0
  let mut theorems := 0
  for (name, info) in env.constants.toList do
    -- Lean emits unsafe compiler IR helpers containing lcProof. They are not
    -- kernel proofs and cannot be used by a safe declaration or theorem.
    if env.getModuleIdxFor? name == some index && !info.isUnsafe then
      declarations := declarations + 1
      if info.isTheorem then theorems := theorems + 1
      for dependency in ← collectAxioms name do
        unless #[``propext, ``Classical.choice, ``Quot.sound].contains dependency do
          throwError "{name} depends on forbidden axiom {dependency}"
  if declarations == 0 then throwError "empty audit: {module.getId}"
  logInfo m!"AUDIT {module.getId}: {declarations} declarations, {theorems} theorems; standard axioms only"
