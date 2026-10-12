use rust_decimal::Decimal;
use stateset_core::{
    CommandEnvelope, ExecutionMode, ExecutionReceipt, ExecutionStatus, InventoryReservation,
    KernelCommandPolicy, KernelPolicy, KernelPrincipal, PrincipalKind, ReservationStatus,
};
use uuid::Uuid;

#[derive(Debug)]
pub(crate) struct Case {
    pub(crate) status: ReservationStatus,
    pub(crate) operation: &'static str,
    pub(crate) due: bool,
    pub(crate) delta: i32,
}

pub(crate) fn cases() -> Vec<Case> {
    let cases: Vec<_> = include_str!("../../../../formal/lean/inventory_preview.golden")
        .lines()
        .map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            assert_eq!(fields.len(), 4);
            Case {
                status: fields[0].parse().expect("status"),
                operation: fields[1],
                due: fields[2].parse().expect("due"),
                delta: fields[3].parse().expect("delta"),
            }
        })
        .collect();
    assert_eq!(cases.len(), 42);
    cases
}

impl Case {
    pub(crate) fn allocated(&self) -> Decimal {
        if self.status.holds_stock() { Decimal::from(2) } else { Decimal::ZERO }
    }

    pub(crate) fn check(
        &self,
        preview: &ExecutionReceipt<InventoryReservation>,
        apply: &ExecutionReceipt<InventoryReservation>,
    ) {
        let rejected = self.operation != "release"
            && (matches!(self.status, ReservationStatus::Released | ReservationStatus::Cancelled)
                || (self.operation == "split" && self.status == ReservationStatus::Confirmed));
        if rejected {
            assert_eq!(preview.status, ExecutionStatus::Rejected, "{self:?}");
            assert_eq!(apply.status, ExecutionStatus::Rejected, "{self:?}");
            assert_eq!(preview.error_code.as_deref(), Some("commerce.reservation_not_confirmable"));
            assert_eq!(apply.error_code, preview.error_code);
            return;
        }
        assert_eq!(preview.status, ExecutionStatus::Previewed, "{self:?}");
        assert_eq!(apply.status, ExecutionStatus::Succeeded, "{self:?}");
        assert_eq!(preview.version_before, Some(10), "{self:?}");
        assert_eq!(preview.version_after, Some(10 + self.delta), "{self:?}");
        assert_eq!(apply.version_before, preview.version_before, "{self:?}");
        assert_eq!(apply.version_after, preview.version_after, "{self:?}");
        assert!(preview.event_ids.is_empty(), "preview emitted events: {self:?}");
        if !self.status.holds_stock()
            || (self.operation == "confirm" && self.status == ReservationStatus::Confirmed)
        {
            assert!(apply.event_ids.is_empty(), "no-op emitted events: {self:?}");
        }
    }
}

pub(crate) fn policy() -> KernelPolicy {
    KernelPolicy::new("formal-preview-v1")
        .allow(
            "inventory.reservation.confirm",
            KernelCommandPolicy::requiring(["inventory.reservation.confirm"]),
        )
        .allow(
            "inventory.reservation.release",
            KernelCommandPolicy::requiring(["inventory.reservation.release"]),
        )
}

pub(crate) fn command<C>(kind: &str, payload: C, mode: ExecutionMode) -> CommandEnvelope<C> {
    let mut command = CommandEnvelope::preview(
        kind,
        Uuid::new_v4().to_string(),
        KernelPrincipal {
            id: "agent:formal-preview".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("formal-tenant".into()),
            delegated_by: Some("user:formal-preview".into()),
            capabilities: vec![kind.into()],
        },
        payload,
    );
    command.mode = mode;
    command.expected_version = Some(10);
    command.store_id = Some("formal-store".into());
    command.policy_version = Some("formal-preview-v1".into());
    command
}
