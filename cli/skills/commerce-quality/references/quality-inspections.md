# Quality Inspections Reference

## Inspection Types

| Type | Trigger | Typical Items |
|------|---------|---------------|
| Incoming | New supplier shipment | Raw materials, components |
| Receiving | PO receipt | All received goods |
| InProcess | During manufacturing | WIP items at checkpoints |
| Final | End of production | Finished goods before storage |
| Random | Periodic audit | Random sample from inventory |
| Return | Customer return | Returned items for condition assessment |

## Inspection Flow

```
Create Inspection
  └─ Schedule (assign inspector, set date)
       └─ Start Inspection
            └─ Record Results per Item
                 ├─ Pass → Release to inventory
                 ├─ Fail → Create NCR, place hold
                 └─ ConditionalPass → Note conditions, may proceed
```

## Non-Conformance Report (NCR)

NCRs track quality failures from discovery to resolution:

| Field | Purpose |
|-------|---------|
| source | Where found: Inspection, CustomerComplaint, InternalAudit, SupplierIssue, ProductionDefect, ShippingDamage |
| severity | Critical, Major, Minor, Observation |
| disposition | What to do: UseAsIs, Rework, Repair, Scrap, ReturnToVendor, Downgrade, SortAndScreen |
| root_cause | Why it happened |
| corrective_action | Immediate fix |
| preventive_action | Prevent recurrence |

## Quality Holds

Prevent inventory movement until quality is verified:

| Hold Type | When Used |
|-----------|-----------|
| QualityInspection | Pending inspection results |
| CustomerReturn | Returned goods assessment |
| Recall | Product recall investigation |
| Damaged | Damage discovered in warehouse |
| Expired | Past expiration date |
| Quarantine | General isolation |
| RegulatoryHold | Regulatory investigation |
| InvestigationHold | Internal investigation |

## Defect Codes

Define reusable defect codes for consistent tracking:
- Code, name, description, severity, category
- Used across inspections for standardized reporting

## Key Metrics

- `pass_rate`: Items passed / total inspected
- `requires_immediate_action`: True if severity is Critical
- `all_items_inspected`: Check before completing inspection
