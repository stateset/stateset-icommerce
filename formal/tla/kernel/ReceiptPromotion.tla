----------------------- MODULE ReceiptPromotion -----------------------
EXTENDS Naturals

(* Two invocations promote one durable preview for the same semantic request.
   Policy, version and seal may change after preview. Seal verification is an
   abstract predicate, not a model of SHA-256 or audit-chain storage. *)
CONSTANTS AtomicPromotion, RecheckGuards, VerifySeal
Workers == {"a", "b"}
VARIABLES stored, allowed, version, sealed, checked, effects, stale, unverified
vars == <<stored, allowed, version, sealed, checked, effects, stale, unverified>>

Init == /\ stored = "previewed" /\ allowed = TRUE /\ version = 0 /\ sealed = TRUE
        /\ checked = [w \in Workers |-> FALSE]
        /\ effects = 0 /\ stale = 0 /\ unverified = 0

CanPromote == stored = "previewed" /\ (~VerifySeal \/ sealed)
GuardsPass == ~RecheckGuards \/ (allowed /\ version = 0)

AtomicApply == /\ AtomicPromotion /\ CanPromote
               /\ stored' = IF GuardsPass THEN "succeeded" ELSE "rejected"
               /\ effects' = effects + IF GuardsPass THEN 1 ELSE 0
               /\ stale' = stale + IF GuardsPass /\ (~allowed \/ version # 0) THEN 1 ELSE 0
               /\ unverified' = unverified + IF GuardsPass /\ ~sealed THEN 1 ELSE 0
               /\ UNCHANGED <<allowed, version, sealed, checked>>

Check(w) == /\ ~AtomicPromotion /\ CanPromote /\ GuardsPass /\ ~checked[w]
            /\ checked' = [checked EXCEPT ![w] = TRUE]
            /\ UNCHANGED <<stored, allowed, version, sealed, effects, stale, unverified>>

Commit(w) == /\ ~AtomicPromotion /\ checked[w]
             /\ stored' = "succeeded"
             /\ checked' = [checked EXCEPT ![w] = FALSE]
             /\ effects' = effects + 1
             /\ stale' = stale + IF ~allowed \/ version # 0 THEN 1 ELSE 0
             /\ unverified' = unverified + IF ~sealed THEN 1 ELSE 0
             /\ UNCHANGED <<allowed, version, sealed>>

Revoke == /\ allowed /\ allowed' = FALSE
          /\ UNCHANGED <<stored, version, sealed, checked, effects, stale, unverified>>
AdvanceVersion == /\ version = 0 /\ version' = 1
                  /\ UNCHANGED <<stored, allowed, sealed, checked, effects, stale, unverified>>
Tamper == /\ sealed /\ sealed' = FALSE
          /\ UNCHANGED <<stored, allowed, version, checked, effects, stale, unverified>>

(* Replays, changed-fingerprint conflicts, and seal errors have no effect. *)
Next == AtomicApply \/ (\E w \in Workers: Check(w) \/ Commit(w))
        \/ Revoke \/ AdvanceVersion \/ Tamper
Spec == Init /\ [][Next]_vars
TypeOK == /\ stored \in {"previewed", "succeeded", "rejected"}
          /\ allowed \in BOOLEAN /\ version \in 0..1 /\ sealed \in BOOLEAN
          /\ checked \in [Workers -> BOOLEAN]
          /\ effects \in 0..2 /\ stale \in 0..2 /\ unverified \in 0..2
AtMostOnce == effects <= 1
NoStaleOrDeniedEffect == stale = 0
NoUnverifiedEffect == unverified = 0
=======================================================================
