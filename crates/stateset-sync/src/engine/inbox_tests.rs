use super::*;
use std::sync::Mutex;

#[derive(Debug)]
struct Source {
    events: Vec<SyncEvent>,
    requests: Mutex<Vec<(u64, usize)>>,
    stalled: bool,
    ignore_limit: bool,
}

impl Source {
    fn new(count: u64) -> Self {
        Self {
            events: (1..=count)
                .map(|sequence| {
                    SyncEvent::new(
                        "updated",
                        "order",
                        sequence.to_string(),
                        serde_json::json!({"remote": sequence}),
                    )
                    .with_remote_sequence(sequence)
                })
                .collect(),
            requests: Mutex::new(Vec::new()),
            stalled: false,
            ignore_limit: false,
        }
    }
}

#[async_trait::async_trait]
impl Transport for Source {
    async fn push_events(&self, events: &[SyncEvent]) -> Result<PushResult, SyncError> {
        Ok(PushResult::accepted_only(events.len(), self.events.len() as u64))
    }

    async fn pull_events(&self, since: u64, limit: usize) -> Result<PullResult, SyncError> {
        self.requests.lock().unwrap().push((since, limit));
        let available: Vec<_> = self.events.iter().filter(|event| event.sequence > since).collect();
        let limit = if self.ignore_limit { available.len() } else { limit };
        Ok(PullResult {
            events: available.iter().take(limit).map(|event| (*event).clone()).collect(),
            remote_head: self.events.len() as u64,
            has_more: available.len() > limit,
        })
    }

    async fn pull_events_page(&self, since: u64, limit: usize) -> Result<PullPage, SyncError> {
        let result = self.pull_events(since, limit).await?;
        let next_cursor =
            if self.stalled { Some(since) } else { derive_next_cursor(since, &result.events) };
        Ok(PullPage { result, next_cursor, observed_cursor: None })
    }
}

fn config(dir: &Path, capacity: usize) -> SyncConfig {
    SyncConfig::new("agent", "tenant", "store")
        .with_outbox_path(dir.join("outbox.json").to_string_lossy())
        .with_state_path(dir.join("state.json").to_string_lossy())
        .with_buffer_capacity(capacity)
}

#[tokio::test]
async fn page_and_cursor_survive_restart_until_idempotent_acknowledgement() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = config(dir.path(), 2);
    let source = Source::new(3);
    let mut engine = SyncEngine::new(cfg.clone()).unwrap();
    engine.pull(&source).await.unwrap();
    drop(engine);
    let mut engine = SyncEngine::new(cfg.clone()).unwrap();
    assert_eq!(engine.buffered_events(), source.events[..2]);
    assert_eq!(engine.state.remote_cursor, 2);
    assert_eq!(engine.next_pull_cursor, Some(2));
    assert!(matches!(engine.pull(&source).await, Err(SyncError::BufferFull(2))));
    assert_eq!(source.requests.lock().unwrap().as_slice(), &[(0, 2)]);
    assert_eq!(engine.acknowledge_buffered_events(&[source.events[0].id]).unwrap(), 1);
    assert_eq!(engine.acknowledge_buffered_events(&[source.events[0].id]).unwrap(), 0);
    engine.pull(&source).await.unwrap();
    assert_eq!(source.requests.lock().unwrap().as_slice(), &[(0, 2), (2, 1)]);
    drop(engine);
    let engine = SyncEngine::new(cfg).unwrap();
    assert_eq!(engine.buffered_events(), source.events[1..]);
    assert_eq!(engine.state.remote_cursor, 3);
}

#[tokio::test]
async fn full_sync_backpressure_keeps_completed_pages_available() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = config(dir.path(), 2);
    let source = Source::new(3);
    let mut engine = SyncEngine::new(cfg.clone()).unwrap();
    assert!(matches!(engine.full_sync(&source).await, Err(SyncError::BufferFull(2))));
    drop(engine);
    let mut engine = SyncEngine::new(cfg).unwrap();
    assert_eq!(engine.buffered_events(), source.events[..2]);
    let applied: Vec<_> = engine.buffered_events().iter().map(|event| event.id).collect();
    engine.acknowledge_buffered_events(&applied).unwrap();
    let (_, page) = engine.full_sync(&source).await.unwrap();
    assert_eq!(page.events, source.events[2..]);
    assert_eq!(engine.buffered_events(), source.events[2..]);
}

#[tokio::test]
async fn failed_page_snapshot_preserves_cursor_inbox_and_local_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = config(dir.path(), 2);
    let source = Source::new(1);
    let mut engine = SyncEngine::with_strategy(cfg.clone(), ConflictStrategy::RemoteWins).unwrap();
    engine
        .record(SyncEvent::new("updated", "order", "1", serde_json::json!({"local": true})))
        .unwrap();
    let before = fs::read(dir.path().join("outbox.json")).unwrap();
    fs::create_dir(dir.path().join("state.json")).unwrap();
    assert!(matches!(engine.pull(&source).await, Err(SyncError::Storage(_))));
    assert_eq!(engine.buffered_count(), 0);
    assert_eq!(engine.state.remote_cursor, 0);
    assert_eq!(engine.next_pull_cursor, None);
    assert_eq!(engine.pending_count(), 1);
    assert_eq!(fs::read(dir.path().join("outbox.json")).unwrap(), before);
    fs::remove_dir(dir.path().join("state.json")).unwrap();
    engine.pull(&source).await.unwrap();
    drop(engine);
    let engine = SyncEngine::new(cfg).unwrap();
    assert_eq!(engine.pending_count(), 0);
    assert_eq!(engine.buffered_events(), source.events);
}

#[tokio::test]
async fn restart_finishes_conflict_cleanup_without_losing_remote_event() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = config(dir.path(), 2);
    let source = Source::new(1);
    let mut engine = SyncEngine::with_strategy(cfg.clone(), ConflictStrategy::RemoteWins).unwrap();
    engine
        .record(SyncEvent::new("updated", "order", "1", serde_json::json!({"local": true})))
        .unwrap();
    let outbox = dir.path().join("outbox.json");
    let saved = dir.path().join("saved.json");
    fs::rename(&outbox, &saved).unwrap();
    fs::create_dir(&outbox).unwrap();
    assert!(engine.pull(&source).await.is_err());
    assert_eq!(engine.buffered_events(), source.events);
    drop(engine);
    fs::remove_dir(&outbox).unwrap();
    fs::rename(saved, outbox).unwrap();
    let mut engine = SyncEngine::new(cfg).unwrap();
    assert_eq!(engine.pending_count(), 0);
    assert_eq!(engine.buffered_events(), source.events);
    assert_eq!(engine.state.remote_cursor, 1);
    assert!(engine.pull(&source).await.unwrap().events.is_empty());
    assert_eq!(engine.buffered_events(), source.events);
}

#[tokio::test]
async fn failed_acknowledgement_and_drain_preserve_pending_events() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = config(dir.path(), 2);
    let source = Source::new(1);
    let mut engine = SyncEngine::new(cfg.clone()).unwrap();
    engine.pull(&source).await.unwrap();
    let state = dir.path().join("state.json");
    let saved = dir.path().join("saved.json");
    fs::rename(&state, &saved).unwrap();
    fs::create_dir(&state).unwrap();
    assert!(engine.acknowledge_buffered_events(&[source.events[0].id]).is_err());
    assert!(engine.try_drain_buffer().is_err());
    assert!(engine.drain_buffer().is_empty());
    assert_eq!(engine.buffered_events(), source.events);
    fs::remove_dir(&state).unwrap();
    fs::rename(saved, state).unwrap();
    drop(engine);
    let mut engine = SyncEngine::new(cfg.clone()).unwrap();
    assert_eq!(engine.try_drain_buffer().unwrap(), source.events);
    drop(engine);
    assert_eq!(SyncEngine::new(cfg).unwrap().buffered_count(), 0);
}

#[tokio::test]
async fn reduced_capacity_preserves_inbox_backlog() {
    let dir = tempfile::tempdir().unwrap();
    let source = Source::new(4);
    let mut engine = SyncEngine::new(config(dir.path(), 3)).unwrap();
    engine.pull(&source).await.unwrap();
    drop(engine);
    let mut engine = SyncEngine::new(config(dir.path(), 1)).unwrap();
    assert_eq!(engine.buffered_events(), source.events[..3]);
    assert!(matches!(engine.pull(&source).await, Err(SyncError::BufferFull(1))));
    engine.acknowledge_buffered_events(&[source.events[0].id, source.events[1].id]).unwrap();
    assert!(matches!(engine.pull(&source).await, Err(SyncError::BufferFull(1))));
    engine.acknowledge_buffered_events(&[source.events[2].id]).unwrap();
    engine.pull(&source).await.unwrap();
    assert_eq!(engine.buffered_events(), source.events[3..]);
}

#[tokio::test]
async fn invalid_pagination_is_rejected_before_mutating_conflicts_or_cursor() {
    let dir = tempfile::tempdir().unwrap();
    let mut source = Source::new(3);
    source.stalled = true;
    let mut engine =
        SyncEngine::with_strategy(config(dir.path(), 2), ConflictStrategy::RemoteWins).unwrap();
    engine
        .record(SyncEvent::new("updated", "order", "1", serde_json::json!({"local": true})))
        .unwrap();
    assert!(matches!(engine.pull(&source).await, Err(SyncError::Transport(_))));
    assert_eq!(engine.pending_count(), 1);
    assert_eq!(engine.buffered_count(), 0);
    assert_eq!(engine.state.remote_cursor, 0);
    assert!(engine.state.last_pull.is_none());
}

#[tokio::test]
async fn oversized_response_cannot_evict_an_unacknowledged_event() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = config(dir.path(), 2);
    let mut source = Source::new(4);
    let mut engine = SyncEngine::new(cfg.clone()).unwrap();
    engine.pull(&source).await.unwrap();
    engine.acknowledge_buffered_events(&[source.events[0].id]).unwrap();
    source.ignore_limit = true;
    assert!(matches!(engine.pull(&source).await, Err(SyncError::Transport(_))));
    assert_eq!(engine.buffered_events(), source.events[1..2]);
    assert_eq!(engine.state.remote_cursor, 2);
    assert_eq!(engine.next_pull_cursor, Some(2));
    drop(engine);
    let engine = SyncEngine::new(cfg).unwrap();
    assert_eq!(engine.buffered_events(), source.events[1..2]);
    assert_eq!(engine.next_pull_cursor, Some(2));
}
