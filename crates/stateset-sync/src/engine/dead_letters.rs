//! Dead-letter queue inspection, requeue, discard, and drain.

use super::*;

impl SyncEngine {
    pub(super) fn dead_letter_index(&self, event_id: Uuid) -> Option<usize> {
        self.dead_letters.iter().position(|dead_letter| dead_letter.event.id == event_id)
    }

    /// Return the number of dead-lettered events retained by the engine.
    #[must_use]
    pub fn dead_letter_count(&self) -> usize {
        self.dead_letters.len()
    }

    /// Return the current dead-letter queue.
    #[must_use]
    pub fn dead_letters(&self) -> &[DeadLetter] {
        &self.dead_letters
    }

    /// Return the retained dead-letter entry for a local event id, if known.
    #[must_use]
    pub fn dead_letter_for_event(&self, event_id: Uuid) -> Option<&DeadLetter> {
        self.dead_letters.iter().find(|dead_letter| dead_letter.event.id == event_id)
    }

    /// Return all retained dead-letter entries associated with a command id.
    #[must_use]
    pub fn dead_letters_for_command(&self, command_id: &str) -> Vec<&DeadLetter> {
        self.dead_letters
            .iter()
            .filter(|dead_letter| dead_letter.event.command_id.as_deref() == Some(command_id))
            .collect()
    }

    /// Return all retained dead-letter entries for an entity identity.
    #[must_use]
    pub fn dead_letters_for_entity(&self, entity_type: &str, entity_id: &str) -> Vec<&DeadLetter> {
        self.dead_letters
            .iter()
            .filter(|dead_letter| {
                dead_letter.event.entity_type == entity_type
                    && dead_letter.event.entity_id == entity_id
            })
            .collect()
    }

    /// Return the latest retained dead-letter entry associated with a command id.
    #[must_use]
    pub fn latest_dead_letter_for_command(&self, command_id: &str) -> Option<&DeadLetter> {
        self.dead_letters
            .iter()
            .filter(|dead_letter| dead_letter.event.command_id.as_deref() == Some(command_id))
            .max_by_key(|dead_letter| dead_letter.rejected_at)
    }

    /// Return the latest retained dead-letter entry for an entity identity.
    #[must_use]
    pub fn latest_dead_letter_for_entity(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Option<&DeadLetter> {
        self.dead_letters
            .iter()
            .filter(|dead_letter| {
                dead_letter.event.entity_type == entity_type
                    && dead_letter.event.entity_id == entity_id
            })
            .max_by_key(|dead_letter| dead_letter.rejected_at)
    }

    /// Requeue a dead-lettered event back into the local outbox.
    ///
    /// Returns the newly assigned local outbox sequence number.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::NotFound`] if the dead-letter entry is missing,
    /// [`SyncError::DuplicateEvent`] if the event is already pending in the
    /// outbox, or [`SyncError::Storage`] if persistence fails.
    pub fn requeue_dead_letter(&mut self, event_id: Uuid) -> Result<u64, SyncError> {
        let Some(index) = self.dead_letter_index(event_id) else {
            return Err(SyncError::NotFound(format!("dead-letter event {event_id}")));
        };
        if self.outbox.contains_event_id(event_id) {
            return Err(SyncError::DuplicateEvent(event_id.to_string()));
        }

        let dead_letter = self.dead_letters[index].clone();
        let sequence = self.outbox.append(dead_letter.event)?;
        let removed = self.dead_letters.remove(index);
        self.state.local_head = self.state.local_head.max(sequence);
        self.state.pending_count = self.outbox.count();

        if let Err(err) = self.persist_runtime_state() {
            self.dead_letters.insert(index, removed);
            if let Err(rollback_err) = self.outbox.try_retain(|event| event.id != event_id) {
                self.state.pending_count = self.outbox.count();
                return Err(SyncError::Storage(format!(
                    "requeue dead-letter rollback failed after snapshot error: snapshot={err}; outbox={rollback_err}"
                )));
            }
            self.state.pending_count = self.outbox.count();
            return Err(err);
        }

        Ok(sequence)
    }

    /// Permanently discard a dead-letter entry.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::NotFound`] if the dead-letter entry is missing or
    /// [`SyncError::Storage`] if persistence fails.
    pub fn discard_dead_letter(&mut self, event_id: Uuid) -> Result<DeadLetter, SyncError> {
        let Some(index) = self.dead_letter_index(event_id) else {
            return Err(SyncError::NotFound(format!("dead-letter event {event_id}")));
        };

        let dead_letter = self.dead_letters.remove(index);
        if let Err(err) = self.persist_runtime_state() {
            self.dead_letters.insert(index, dead_letter);
            return Err(err);
        }
        Ok(dead_letter)
    }

    /// Drain all retained dead-letter entries and persist the updated state.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Storage`] if the runtime state snapshot cannot be updated.
    pub fn drain_dead_letters(&mut self) -> Result<Vec<DeadLetter>, SyncError> {
        let dead_letters = std::mem::take(&mut self.dead_letters);
        if let Err(err) = self.persist_runtime_state() {
            self.dead_letters = dead_letters;
            return Err(err);
        }
        Ok(dead_letters)
    }
}
