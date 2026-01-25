# Domain Module Consolidation Plan

## Current State (32 modules → 16 modules)

### Financial Module Group (7 → 2)
**Keep:**
- `finance.rs` - Merge AP, AR, GL, credit
- `cost_accounting.rs` - Keep separate for manufacturing complexity

**Merge into `finance.rs`:**
- `accounts_payable.rs`
- `accounts_receivable.rs`
- `general_ledger.rs`
- `credit.rs`

### Logistics Module Group (6 → 3)
**Keep:**
- `inventory.rs`
- `manufacturing.rs` - Merge BOM, work orders, quality
- `logistics.rs` - Merge shipments, warehouse, receiving, lots, serials

**Merge into `logistics.rs`:**
- `shipments.rs`
- `warehouse.rs`
- `receiving.rs`
- `lots.rs`
- `serials.rs`

**Merge into `manufacturing.rs`:**
- `bom.rs`
- `work_orders.rs`
- `quality.rs`

### Commerce Module Group (4 → 2)
**Keep:**
- `commerce.rs` - Merge orders, payments, returns
- `catalog.rs` - Merge products, carts

**Merge into `commerce.rs`:**
- `order.rs`
- `payment.rs`
- `returns.rs`

**Merge into `catalog.rs`:**
- `product.rs`
- `cart.rs`

### Operational Module Group (3 → 2)
**Keep:**
- `operations.rs` - Merge subscriptions, invoices, purchase orders
- `analytics.rs` - Keep separate

**Merge into `operations.rs`:**
- `subscription.rs`
- `invoice.rs`
- `purchase_order.rs`

### Standalone Modules (8 → 5)
**Keep:**
- `customer.rs`
- `promotion.rs`
- `tax.rs`
- `warranty.rs`
- `backorder.rs`

**Deprecate:**
- `fulfillment.rs` - Fold into logistics

## Migration Strategy

### Phase 1: Create New Modules (Week 1)
1. Create `finance.rs` with all financial types
2. Create `logistics.rs` with all logistics types
3. Create `manufacturing.rs` with all manufacturing types
4. Create `commerce.rs` with all commerce types
5. Create `catalog.rs` with all catalog types
6. Create `operations.rs` with all operations types

### Phase 2: Re-Export from Old Modules (Week 1)
```rust
// accounts_payable.rs
pub use crate::finance::*;
```

### Phase 3: Update Imports (Week 2)
1. Search/replace imports across codebase
2. Update binding generators
3. Update examples
4. Update tests

### Phase 4: Remove Old Modules (Week 2)
1. Verify all imports updated
2. Remove old module files
3. Run full test suite

## Benefits

- **Reduced cognitive load**: 16 modules easier to navigate than 32
- **Better organization**: Logical grouping by business domain
- **Fewer files**: Less context switching
- **Easier testing**: Related code in same module
- **Better performance**: Fewer module lookups

## Files to Create

```
crates/stateset-core/src/
├── finance.rs          (new - 2,000 lines)
├── logistics.rs        (new - 2,500 lines)
├── manufacturing.rs    (new - 1,800 lines)
├── commerce.rs         (new - 1,500 lines)
├── catalog.rs          (new - 1,200 lines)
└── operations.rs       (new - 1,400 lines)
```

## Files to Deprecate

```
accounts_payable.rs → finance.rs
accounts_receivable.rs → finance.rs
general_ledger.rs → finance.rs
credit.rs → finance.rs
bom.rs → manufacturing.rs
work_orders.rs → manufacturing.rs
quality.rs → manufacturing.rs
shipments.rs → logistics.rs
warehouse.rs → logistics.rs
receiving.rs → logistics.rs
lots.rs → logistics.rs
serials.rs → logistics.rs
order.rs → commerce.rs
payment.rs → commerce.rs
returns.rs → commerce.rs
product.rs → catalog.rs
cart.rs → catalog.rs
subscription.rs → operations.rs
invoice.rs → operations.rs
purchase_order.rs → operations.rs
fulfillment.rs → logistics.rs
```

## Testing Strategy

1. **Keep old modules** during migration
2. **Test new modules** comprehensively
3. **Re-export** from old to new
4. **Verify** no breaking changes
5. **Remove** old modules after verification

## Rollback Plan

If issues arise:
1. Keep old modules in version control
2. Can revert to old structure in < 1 hour
3. Feature flag new module structure
4. Gradual rollout via configuration