//! Outbox -> remote push, push-result validation, and confirmation retention.

use super::*;

impl SyncEngine {
    /// Push pending events from the outbox to the remote via the given transport.
    ///
    /// Drains up to `batch_size` events from the outbox.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Transport`] if the transport operation fails.
    pub async fn push(&mut self, transport: &dyn Transport) -> Result<PushResult, SyncError> {
        let batch_size = self.config.resolved_batch_size();
        let events: Vec<SyncEvent> = self.outbox.peek(batch_size).into_iter().cloned().collect();

        if events.is_empty() {
            return Ok(PushResult::accepted_only(0, self.state.remote_head));
        }

        let result = transport.push_events(&events).await?;
        Self::validate_push_result(&events, &result)?;

        let dead_letters = Self::collect_dead_letters(&events, &result.rejections);
        let dead_letter_ids: HashSet<_> = dead_letters.iter().map(|entry| entry.event.id).collect();
        let removable_ids: HashSet<_> = if result.acknowledgements.is_empty() {
            events
                .iter()
                .take(result.accepted)
                .map(|event| event.id)
                .chain(dead_letter_ids.iter().copied())
                .collect()
        } else {
            result
                .acknowledgements
                .iter()
                .map(|ack| ack.event_id)
                .chain(dead_letter_ids.iter().copied())
                .collect()
        };

        if !removable_ids.is_empty() {
            if let Err(err) = self.outbox.try_retain(|event| !removable_ids.contains(&event.id)) {
                self.state.pending_count = self.outbox.count();
                return Err(err);
            }
        }

        if !result.acknowledgements.is_empty() {
            self.retain_push_confirmations(&events, &result.acknowledgements);
        }

        if !dead_letters.is_empty() {
            self.dead_letters.extend(dead_letters);
        }

        self.state.remote_head = self.state.remote_head.max(result.remote_head);
        if let Some(acknowledged_head) = result.acknowledged_head() {
            self.state.last_acknowledged_remote_sequence = Some(
                self.state
                    .last_acknowledged_remote_sequence
                    .map_or(acknowledged_head, |current| current.max(acknowledged_head)),
            );
        }
        self.state.last_push = Some(Utc::now());
        self.state.pending_count = self.outbox.count();
        self.persist_runtime_state()?;

        Ok(result)
    }

    pub(super) fn collect_dead_letters(
        events: &[SyncEvent],
        rejections: &[PushRejection],
    ) -> Vec<DeadLetter> {
        let events_by_id: HashMap<_, _> = events.iter().map(|event| (event.id, event)).collect();
        rejections
            .iter()
            .filter(|rejection| rejection.retryable != Some(true))
            .filter_map(|rejection| {
                events_by_id
                    .get(&rejection.event_id)
                    .map(|event| DeadLetter::new((*event).clone(), rejection.clone()))
            })
            .collect()
    }

    pub(super) fn collect_push_confirmations(
        events: &[SyncEvent],
        acknowledgements: &[PushAcknowledgement],
    ) -> Vec<PushConfirmation> {
        let events_by_id: HashMap<_, _> = events.iter().map(|event| (event.id, event)).collect();
        acknowledgements
            .iter()
            .filter_map(|acknowledgement| {
                events_by_id
                    .get(&acknowledgement.event_id)
                    .map(|event| PushConfirmation::from_ack(event, acknowledgement))
            })
            .collect()
    }

    pub(super) fn retain_push_confirmations(
        &mut self,
        events: &[SyncEvent],
        acknowledgements: &[PushAcknowledgement],
    ) {
        for confirmation in Self::collect_push_confirmations(events, acknowledgements) {
            self.confirmations.retain(|existing| {
                existing.event_id != confirmation.event_id
                    && existing.remote_sequence != confirmation.remote_sequence
            });
            self.confirmations.push(confirmation);
        }
        self.trim_confirmations_to_capacity();
    }

    pub(super) fn trim_confirmations_to_capacity(&mut self) {
        let capacity = self.config.resolved_confirmation_capacity();
        if self.confirmations.len() > capacity {
            let overflow = self.confirmations.len() - capacity;
            self.confirmations.drain(0..overflow);
        }
    }

    pub(super) fn validate_push_result(
        events: &[SyncEvent],
        result: &PushResult,
    ) -> Result<(), SyncError> {
        if result.accepted > events.len() {
            return Err(SyncError::Transport(format!(
                "push acknowledged more events than were sent (sent={}, accepted={})",
                events.len(),
                result.accepted
            )));
        }

        if result.accepted + result.rejections.len() > events.len() {
            return Err(SyncError::Transport(format!(
                "push reported more terminal outcomes than were sent (sent={}, accepted={}, rejected={})",
                events.len(),
                result.accepted,
                result.rejections.len()
            )));
        }

        if !result.acknowledgements.is_empty() && result.acknowledgements.len() != result.accepted {
            return Err(SyncError::Transport(format!(
                "push acknowledgements did not match accepted count (accepted={}, acknowledgements={})",
                result.accepted,
                result.acknowledgements.len()
            )));
        }

        let expected_ids: HashSet<_> = events.iter().map(|event| event.id).collect();
        let mut seen_ids =
            HashSet::with_capacity(result.acknowledgements.len() + result.rejections.len());

        for acknowledgement in &result.acknowledgements {
            if acknowledgement.remote_sequence == 0 {
                return Err(SyncError::Transport(
                    "push acknowledgement contained remote_sequence=0".to_string(),
                ));
            }

            if !expected_ids.contains(&acknowledgement.event_id) {
                return Err(SyncError::Transport(
                    "push acknowledgement did not match a sent local event".to_string(),
                ));
            }

            if !seen_ids.insert(acknowledgement.event_id) {
                return Err(SyncError::Transport(
                    "push acknowledgement contained duplicate event ids".to_string(),
                ));
            }
        }

        for rejection in &result.rejections {
            if !expected_ids.contains(&rejection.event_id) {
                return Err(SyncError::Transport(
                    "push rejection did not match a sent local event".to_string(),
                ));
            }

            if !seen_ids.insert(rejection.event_id) {
                return Err(SyncError::Transport(
                    "push result contained duplicate event ids across acknowledgements/rejections"
                        .to_string(),
                ));
            }
        }

        if result.acknowledgements.is_empty() && !result.rejections.is_empty() {
            let assumed_accepted_prefix: HashSet<_> =
                events.iter().take(result.accepted).map(|event| event.id).collect();
            if result
                .rejections
                .iter()
                .any(|rejection| assumed_accepted_prefix.contains(&rejection.event_id))
            {
                return Err(SyncError::Transport(
                    "push rejection overlapped the accepted prefix without per-event acknowledgements"
                        .to_string(),
                ));
            }
        }

        if let Some(acknowledged_head) = result.acknowledged_head() {
            if acknowledged_head > result.remote_head {
                return Err(SyncError::Transport(format!(
                    "push acknowledgement head exceeded remote head (ack_head={}, remote_head={})",
                    acknowledged_head, result.remote_head
                )));
            }
        }

        Ok(())
    }
}
