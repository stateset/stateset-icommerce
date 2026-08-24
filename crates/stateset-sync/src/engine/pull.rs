//! Remote -> buffer pull with conflict resolution, pagination, and `full_sync`.

use super::*;

/// Safety stop for paginated pull loops in `full_sync`.
const MAX_PULL_PAGES: usize = 10_000;

impl SyncEngine {
    /// Pull events from the remote sequencer into the local buffer.
    ///
    /// Pulled events are added to the event buffer. If conflicts are
    /// detected (same `entity_type` + `entity_id` in both outbox and pulled),
    /// they are resolved using the configured strategy.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Transport`] if the transport operation fails.
    pub async fn pull(&mut self, transport: &dyn Transport) -> Result<PullResult, SyncError> {
        let since = self.next_pull_cursor.unwrap_or(self.state.remote_cursor);
        let (result, next_cursor) = self.pull_since(transport, since).await?;
        self.next_pull_cursor = next_cursor;
        self.persist_runtime_state()?;
        Ok(result)
    }

    pub(super) async fn pull_since(
        &mut self,
        transport: &dyn Transport,
        since: u64,
    ) -> Result<(PullResult, Option<u64>), SyncError> {
        let limit = self.config.resolved_batch_size();
        let PullPage {
            result,
            next_cursor: transport_next_cursor,
            observed_cursor: transport_observed_cursor,
        } = transport.pull_events_page(since, limit).await?;

        // Detect and resolve conflicts between pending outbox events and pulled events
        let pending: Vec<SyncEvent> =
            self.outbox.peek(self.outbox.count()).into_iter().cloned().collect();
        let mut drop_local_ids = HashSet::new();
        let mut events_to_buffer = Vec::with_capacity(result.events.len());

        for pulled_event in &result.events {
            let mut keep_remote = true;

            if let Some(local_event) = pending.iter().rev().find(|local_event| {
                local_event.entity_type == pulled_event.entity_type
                    && local_event.entity_id == pulled_event.entity_id
            }) {
                match self.resolver.resolve(local_event, pulled_event) {
                    Resolution::KeepLocal => {
                        keep_remote = false;
                    }
                    Resolution::KeepRemote => {
                        drop_local_ids.insert(local_event.id);
                    }
                    Resolution::Merge(merged) => {
                        drop_local_ids.insert(local_event.id);
                        events_to_buffer.push(*merged);
                        keep_remote = false;
                    }
                }
            }

            if keep_remote {
                events_to_buffer.push(pulled_event.clone());
            }
        }

        if !drop_local_ids.is_empty() {
            self.outbox.try_retain(|event| !drop_local_ids.contains(&event.id))?;
            self.state.pending_count = self.outbox.count();
        }

        // Buffer resolved events
        for event in events_to_buffer {
            self.buffer.push(event);
        }

        self.state.remote_head = result.remote_head;
        self.state.last_pull = Some(Utc::now());
        let observed_cursor = transport_observed_cursor
            .filter(|cursor| *cursor > since)
            .or_else(|| derive_next_cursor(since, &result.events))
            .unwrap_or(since);
        self.state.remote_cursor = self.state.remote_cursor.max(observed_cursor);

        let next_cursor = Self::resolve_next_cursor(
            since,
            &result.events,
            result.has_more,
            transport_next_cursor,
        )?;

        Ok((result, next_cursor))
    }

    pub(super) fn resolve_next_cursor(
        since: u64,
        events: &[SyncEvent],
        has_more: bool,
        transport_next_cursor: Option<u64>,
    ) -> Result<Option<u64>, SyncError> {
        if !has_more {
            return Ok(None);
        }

        let next_cursor = transport_next_cursor.or_else(|| derive_next_cursor(since, events));
        let Some(next_cursor) = next_cursor else {
            return Err(SyncError::Transport(
                "pull pagination stalled: has_more=true but no advancing cursor available"
                    .to_string(),
            ));
        };
        if next_cursor <= since {
            return Err(SyncError::Transport(format!(
                "pull pagination cursor did not advance (since={since}, next_cursor={next_cursor})"
            )));
        }
        Ok(Some(next_cursor))
    }

    /// Perform a full sync: push first, then pull.
    ///
    /// # Errors
    ///
    /// Returns the first error encountered during push or pull.
    pub async fn full_sync(
        &mut self,
        transport: &dyn Transport,
    ) -> Result<(PushResult, PullResult), SyncError> {
        let push_result = self.push(transport).await?;
        let mut since = self.next_pull_cursor.unwrap_or(self.state.remote_cursor);
        let (mut pull_result, next_cursor) = self.pull_since(transport, since).await?;
        self.next_pull_cursor = next_cursor;
        self.persist_runtime_state()?;
        let mut pull_pages = 1;

        while pull_result.has_more {
            if pull_pages >= MAX_PULL_PAGES {
                return Err(SyncError::Transport(
                    "pull pagination exceeded safety limit".to_string(),
                ));
            }

            since = self.next_pull_cursor.ok_or_else(|| {
                SyncError::Transport(
                    "pull pagination stalled: has_more=true but no continuation cursor".to_string(),
                )
            })?;

            let (next_page, page_cursor) = self.pull_since(transport, since).await?;
            self.next_pull_cursor = page_cursor;
            self.persist_runtime_state()?;

            pull_result.events.extend(next_page.events);
            pull_result.remote_head = next_page.remote_head;
            pull_result.has_more = next_page.has_more;
            pull_pages += 1;
        }

        self.next_pull_cursor = None;
        self.persist_runtime_state()?;
        Ok((push_result, pull_result))
    }
}
