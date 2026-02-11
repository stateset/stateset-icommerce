//! Return operations

use stateset_core::{CreateReturn, Result, Return, ReturnFilter, ReturnStatus, UpdateReturn};
use stateset_db::Database;
use stateset_observability::Metrics;
use std::sync::Arc;
use uuid::Uuid;

#[cfg(feature = "events")]
use crate::events::EventSystem;
#[cfg(feature = "events")]
use rust_decimal::Decimal;
#[cfg(feature = "events")]
use stateset_core::CommerceEvent;

/// Return operations interface.
pub struct Returns {
    db: Arc<dyn Database>,
    metrics: Metrics,
    #[cfg(feature = "events")]
    event_system: Arc<EventSystem>,
}

impl Returns {
    #[cfg(feature = "events")]
    pub(crate) fn new(
        db: Arc<dyn Database>,
        event_system: Arc<EventSystem>,
        metrics: Metrics,
    ) -> Self {
        Self {
            db,
            metrics,
            event_system,
        }
    }

    #[cfg(not(feature = "events"))]
    pub(crate) fn new(db: Arc<dyn Database>, metrics: Metrics) -> Self {
        Self { db, metrics }
    }

    #[cfg(feature = "events")]
    fn emit(&self, event: CommerceEvent) {
        self.event_system.emit(event);
    }

    #[cfg(feature = "events")]
    fn emit_status_change(&self, previous: &Return, updated: &Return) {
        if previous.status != updated.status {
            self.emit(CommerceEvent::ReturnStatusChanged {
                return_id: updated.id,
                from_status: previous.status,
                to_status: updated.status,
                timestamp: updated.updated_at,
            });
        }
    }

    /// Create a new return request.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let ret = commerce.returns().create(CreateReturn {
    ///     order_id: uuid::Uuid::new_v4(),
    ///     reason: ReturnReason::Defective,
    ///     items: vec![CreateReturnItem {
    ///         order_item_id: uuid::Uuid::new_v4(),
    ///         quantity: 1,
    ///         ..Default::default()
    ///     }],
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn create(&self, input: CreateReturn) -> Result<Return> {
        let ret = self.db.returns().create(input)?;
        self.metrics.record_return_requested(&ret.id.to_string());
        #[cfg(feature = "events")]
        {
            self.emit(CommerceEvent::ReturnRequested {
                return_id: ret.id,
                order_id: ret.order_id,
                customer_id: ret.customer_id,
                reason: ret.reason,
                item_count: ret.items.len(),
                timestamp: ret.created_at,
            });
        }
        Ok(ret)
    }

    /// Get a return by ID.
    pub fn get(&self, id: Uuid) -> Result<Option<Return>> {
        self.db.returns().get(id)
    }

    /// Update a return.
    pub fn update(&self, id: Uuid, input: UpdateReturn) -> Result<Return> {
        #[cfg(feature = "events")]
        let previous = self.db.returns().get(id)?;

        let updated = self.db.returns().update(id, input)?;

        #[cfg(feature = "events")]
        if let Some(previous) = previous {
            if previous.status != updated.status {
                self.emit_status_change(&previous, &updated);
                match updated.status {
                    ReturnStatus::Approved => {
                        self.emit(CommerceEvent::ReturnApproved {
                            return_id: updated.id,
                            order_id: updated.order_id,
                            timestamp: updated.updated_at,
                        });
                    }
                    ReturnStatus::Rejected => {
                        self.emit(CommerceEvent::ReturnRejected {
                            return_id: updated.id,
                            order_id: updated.order_id,
                            reason: updated.notes.clone().unwrap_or_default(),
                            timestamp: updated.updated_at,
                        });
                    }
                    ReturnStatus::Completed => {
                        let refund_amount = updated.refund_amount.unwrap_or(Decimal::ZERO);
                        self.emit(CommerceEvent::ReturnCompleted {
                            return_id: updated.id,
                            order_id: updated.order_id,
                            refund_amount,
                            timestamp: updated.updated_at,
                        });
                        if let (Some(amount), Some(method)) =
                            (updated.refund_amount, updated.refund_method.clone())
                        {
                            self.emit(CommerceEvent::RefundIssued {
                                return_id: updated.id,
                                order_id: updated.order_id,
                                amount,
                                method,
                                timestamp: updated.updated_at,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(updated)
    }

    /// List returns with optional filtering.
    pub fn list(&self, filter: ReturnFilter) -> Result<Vec<Return>> {
        self.db.returns().list(filter)
    }

    /// List returns for a specific order.
    pub fn list_for_order(&self, order_id: Uuid) -> Result<Vec<Return>> {
        self.db.returns().list(ReturnFilter {
            order_id: Some(order_id),
            ..Default::default()
        })
    }

    /// List returns for a specific customer.
    pub fn list_for_customer(&self, customer_id: Uuid) -> Result<Vec<Return>> {
        self.db.returns().list(ReturnFilter {
            customer_id: Some(customer_id),
            ..Default::default()
        })
    }

    /// Approve a return request.
    pub fn approve(&self, id: Uuid) -> Result<Return> {
        #[cfg(feature = "events")]
        let previous = self.db.returns().get(id)?;

        let ret = self.db.returns().approve(id)?;

        #[cfg(feature = "events")]
        {
            if let Some(previous) = previous {
                self.emit_status_change(&previous, &ret);
            }
            self.emit(CommerceEvent::ReturnApproved {
                return_id: ret.id,
                order_id: ret.order_id,
                timestamp: ret.updated_at,
            });
        }

        Ok(ret)
    }

    /// Reject a return request.
    pub fn reject(&self, id: Uuid, reason: &str) -> Result<Return> {
        #[cfg(feature = "events")]
        let previous = self.db.returns().get(id)?;

        let ret = self.db.returns().reject(id, reason)?;

        #[cfg(feature = "events")]
        {
            if let Some(previous) = previous {
                self.emit_status_change(&previous, &ret);
            }
            self.emit(CommerceEvent::ReturnRejected {
                return_id: ret.id,
                order_id: ret.order_id,
                reason: reason.to_string(),
                timestamp: ret.updated_at,
            });
        }

        Ok(ret)
    }

    /// Mark a return as received.
    pub fn mark_received(&self, id: Uuid) -> Result<Return> {
        self.update(
            id,
            UpdateReturn {
                status: Some(ReturnStatus::Received),
                ..Default::default()
            },
        )
    }

    /// Complete a return (process refund).
    pub fn complete(&self, id: Uuid) -> Result<Return> {
        #[cfg(feature = "events")]
        let previous = self.db.returns().get(id)?;

        let ret = self.db.returns().complete(id)?;

        #[cfg(feature = "events")]
        {
            if let Some(previous) = previous {
                self.emit_status_change(&previous, &ret);
            }
            let refund_amount = ret.refund_amount.unwrap_or(Decimal::ZERO);
            self.emit(CommerceEvent::ReturnCompleted {
                return_id: ret.id,
                order_id: ret.order_id,
                refund_amount,
                timestamp: ret.updated_at,
            });
            if let (Some(amount), Some(method)) = (ret.refund_amount, ret.refund_method.clone()) {
                self.emit(CommerceEvent::RefundIssued {
                    return_id: ret.id,
                    order_id: ret.order_id,
                    amount,
                    method,
                    timestamp: ret.updated_at,
                });
            }
        }

        Ok(ret)
    }

    /// Cancel a return.
    pub fn cancel(&self, id: Uuid) -> Result<Return> {
        #[cfg(feature = "events")]
        let previous = self.db.returns().get(id)?;

        let ret = self.db.returns().cancel(id)?;

        #[cfg(feature = "events")]
        if let Some(previous) = previous {
            self.emit_status_change(&previous, &ret);
        }

        Ok(ret)
    }

    /// Count returns matching a filter.
    pub fn count(&self, filter: ReturnFilter) -> Result<u64> {
        self.db.returns().count(filter)
    }

    /// Add tracking number to a return.
    pub fn add_tracking(&self, id: Uuid, tracking_number: &str) -> Result<Return> {
        self.update(
            id,
            UpdateReturn {
                tracking_number: Some(tracking_number.to_string()),
                status: Some(ReturnStatus::InTransit),
                ..Default::default()
            },
        )
    }

    /// List pending returns (awaiting approval).
    pub fn list_pending(&self) -> Result<Vec<Return>> {
        self.db.returns().list(ReturnFilter {
            status: Some(ReturnStatus::Requested),
            ..Default::default()
        })
    }
}
