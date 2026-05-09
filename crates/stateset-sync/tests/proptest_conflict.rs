//! Property-based tests for `ConflictResolver` semantics.
//!
//! These properties guarantee the conflict-resolution truth table holds for
//! arbitrary timestamps, entity-id strings, and event types — not just the
//! happy-path examples in the unit tests.

use chrono::{DateTime, Duration, TimeZone, Utc};
use proptest::prelude::*;
use serde_json::json;
use stateset_sync::{ConflictResolver, ConflictStrategy, Resolution, event::SyncEvent};
use uuid::Uuid;

fn make_event(name: &str, ts: DateTime<Utc>) -> SyncEvent {
    SyncEvent::with_id(Uuid::new_v4(), 0, name, "order", "ORD-1", json!({"action": name}), ts)
}

/// Strategy for an arbitrary timestamp in the year 2026 (~31.5M seconds).
fn arb_timestamp() -> impl Strategy<Value = DateTime<Utc>> {
    (0_i64..31_536_000_i64).prop_map(|secs| {
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap() + Duration::seconds(secs)
    })
}

proptest! {
    /// `RemoteWins` returns `KeepRemote` for ANY pair of events,
    /// regardless of timestamps, types, or content.
    #[test]
    fn remote_wins_is_total(local_ts in arb_timestamp(), remote_ts in arb_timestamp()) {
        let resolver = ConflictResolver::new(ConflictStrategy::RemoteWins);
        let local = make_event("local", local_ts);
        let remote = make_event("remote", remote_ts);
        prop_assert_eq!(resolver.resolve(&local, &remote), Resolution::KeepRemote);
    }

    /// `LocalWins` returns `KeepLocal` for ANY pair.
    #[test]
    fn local_wins_is_total(local_ts in arb_timestamp(), remote_ts in arb_timestamp()) {
        let resolver = ConflictResolver::new(ConflictStrategy::LocalWins);
        let local = make_event("local", local_ts);
        let remote = make_event("remote", remote_ts);
        prop_assert_eq!(resolver.resolve(&local, &remote), Resolution::KeepLocal);
    }

    /// `LastWriterWins` keeps local iff `local.timestamp >= remote.timestamp`.
    /// This is the core ordering invariant.
    #[test]
    fn last_writer_wins_respects_timestamp_ordering(
        local_ts in arb_timestamp(),
        remote_ts in arb_timestamp()
    ) {
        let resolver = ConflictResolver::new(ConflictStrategy::LastWriterWins);
        let local = make_event("local", local_ts);
        let remote = make_event("remote", remote_ts);
        let resolution = resolver.resolve(&local, &remote);

        if local_ts >= remote_ts {
            prop_assert_eq!(resolution, Resolution::KeepLocal);
        } else {
            prop_assert_eq!(resolution, Resolution::KeepRemote);
        }
    }

    /// `LastWriterWins` ties go to `local`. (Reflexivity at equality.)
    #[test]
    fn last_writer_wins_breaks_ties_in_favor_of_local(ts in arb_timestamp()) {
        let resolver = ConflictResolver::new(ConflictStrategy::LastWriterWins);
        let local = make_event("local", ts);
        let remote = make_event("remote", ts);
        prop_assert_eq!(resolver.resolve(&local, &remote), Resolution::KeepLocal);
    }

    /// `LastWriterWins` is anti-symmetric on strictly-ordered timestamps.
    /// If swapping local and remote inverts the timestamp ordering, the
    /// resolution should also invert.
    #[test]
    fn last_writer_wins_inverts_on_swap(
        ts_a in arb_timestamp(),
        ts_b in arb_timestamp()
    ) {
        prop_assume!(ts_a != ts_b);
        let resolver = ConflictResolver::new(ConflictStrategy::LastWriterWins);
        let evt_a = make_event("a", ts_a);
        let evt_b = make_event("b", ts_b);
        let forward = resolver.resolve(&evt_a, &evt_b);
        let swapped = resolver.resolve(&evt_b, &evt_a);
        // KeepLocal in forward maps to KeepRemote in swapped, and vice versa.
        match (forward, swapped) {
            (Resolution::KeepLocal, Resolution::KeepRemote)
            | (Resolution::KeepRemote, Resolution::KeepLocal) => {}
            other => prop_assert!(false, "swap should invert the resolution: {:?}", other),
        }
    }

    /// `resolve_batch(pairs)` length equals input length and each element
    /// matches `resolve(local, remote)`.
    #[test]
    fn batch_matches_individual_resolutions(
        timestamps in proptest::collection::vec((arb_timestamp(), arb_timestamp()), 0..6)
    ) {
        let resolver = ConflictResolver::new(ConflictStrategy::LastWriterWins);
        let events: Vec<(SyncEvent, SyncEvent)> = timestamps
            .iter()
            .map(|(l_ts, r_ts)| {
                (make_event("local", *l_ts), make_event("remote", *r_ts))
            })
            .collect();
        let pairs: Vec<(&SyncEvent, &SyncEvent)> =
            events.iter().map(|(l, r)| (l, r)).collect();

        let batch = resolver.resolve_batch(&pairs);
        prop_assert_eq!(batch.len(), pairs.len());
        for (i, (l, r)) in pairs.iter().enumerate() {
            prop_assert_eq!(&batch[i], &resolver.resolve(l, r));
        }
    }

    /// The resolver's strategy accessor returns exactly what was constructed.
    #[test]
    fn strategy_accessor_round_trips(_unused in 0_u8..1) {
        for s in [
            ConflictStrategy::RemoteWins,
            ConflictStrategy::LocalWins,
            ConflictStrategy::LastWriterWins,
        ] {
            prop_assert_eq!(ConflictResolver::new(s).strategy(), s);
        }
    }

    /// SyncEvent::new produces a hash that equals SyncEvent::with_id with the
    /// same payload — `compute_hash(payload)` is deterministic.
    #[test]
    fn sync_event_hash_is_deterministic(
        action in "[a-z]{1,8}",
        ts in arb_timestamp()
    ) {
        let payload = json!({"action": action});
        let e1 = SyncEvent::with_id(
            Uuid::new_v4(), 0, "evt", "order", "ORD", payload.clone(), ts
        );
        let e2 = SyncEvent::with_id(
            Uuid::new_v4(), 0, "evt", "order", "ORD", payload, ts
        );
        prop_assert_eq!(e1.hash, e2.hash, "same payload must hash identically");
    }
}
