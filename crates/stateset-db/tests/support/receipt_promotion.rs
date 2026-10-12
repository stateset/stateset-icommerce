use rust_decimal::Decimal;
use stateset_core::{
    CommandEnvelope, ConfirmInventoryReservation, CreateInventoryItem, ExecutionMode,
    KernelCommandPolicy, KernelPolicy, KernelPrincipal, PrincipalKind, ReserveInventory,
};
use uuid::Uuid;

pub(crate) fn policy(allowed: bool) -> KernelPolicy {
    let policy = KernelPolicy::new("formal-promotion-v1");
    if allowed {
        policy.allow(
            "inventory.reservation.confirm",
            KernelCommandPolicy::requiring(["inventory.reservation.confirm"]),
        )
    } else {
        policy
    }
}

pub(crate) fn item(sku: &str) -> CreateInventoryItem {
    CreateInventoryItem {
        sku: sku.into(),
        name: "Receipt promotion".into(),
        initial_quantity: Some(Decimal::from(10)),
        ..Default::default()
    }
}

pub(crate) fn reservation(sku: &str, quantity: i32) -> ReserveInventory {
    ReserveInventory {
        sku: sku.into(),
        location_id: Some(1),
        quantity: Decimal::from(quantity),
        reference_type: "formal".into(),
        reference_id: sku.into(),
        expires_in_seconds: None,
    }
}

pub(crate) fn command(id: Uuid) -> CommandEnvelope<ConfirmInventoryReservation> {
    let kind = "inventory.reservation.confirm";
    let mut command = CommandEnvelope::preview(
        kind,
        Uuid::new_v4().to_string(),
        KernelPrincipal {
            id: "agent:formal-promotion".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("formal-tenant".into()),
            delegated_by: Some("user:formal".into()),
            capabilities: vec![kind.into()],
        },
        ConfirmInventoryReservation { reservation_id: id, quantity: Some(Decimal::ONE) },
    );
    command.store_id = Some("formal-store".into());
    command.policy_version = Some("formal-promotion-v1".into());
    command.expected_version = Some(2);
    command
}

pub(crate) fn promote(command: &mut CommandEnvelope<ConfirmInventoryReservation>) {
    command.mode = ExecutionMode::Apply;
    command.command_id = Uuid::new_v4();
}
