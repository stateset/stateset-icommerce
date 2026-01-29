# Cost Accounting Methods Reference

## Costing Methods Comparison

| Method | Best For | How It Works |
|--------|----------|-------------|
| Average | General merchandise | Running weighted average of all receipts |
| FIFO | Perishables, regulated | Consume oldest cost layers first |
| LIFO | Tax optimization (US) | Consume newest cost layers first |
| Standard | Manufacturing | Predetermined costs, track variances |
| Specific | High-value items | Track actual cost per unit |

## Item Cost Components

| Component | Description |
|-----------|-------------|
| Material | Raw material or purchase cost |
| Labor | Direct labor cost |
| Overhead | Allocated overhead |
| **Unit Cost** | Sum of all components |

## Cost Layers (FIFO/LIFO)

Each receipt creates a new cost layer:
- `layer_date`: When inventory was received
- `source`: Purchase, Production, Transfer, Adjustment, Opening
- `quantity_initial` / `quantity_remaining`: Layer quantities
- `unit_cost`: Cost per unit for this layer
- `total_cost`: quantity_remaining * unit_cost

### FIFO Consumption
```
Layer 1: 100 units @ $10 (oldest - consume first)
Layer 2: 50 units @ $11
Layer 3: 75 units @ $10.50 (newest)

Issue 120 units:
  100 from Layer 1 @ $10 = $1,000
  20 from Layer 2 @ $11 = $220
  Total COGS = $1,220
```

### LIFO Consumption
```
Same layers, issue 120 units:
  75 from Layer 3 @ $10.50 = $787.50
  45 from Layer 2 @ $11 = $495
  Total COGS = $1,282.50
```

## Variance Analysis

### Standard Cost Variances

| Variance | Formula |
|----------|---------|
| Purchase Price | (Actual Price - Standard Price) x Actual Qty |
| Material Usage | (Actual Qty - Standard Qty) x Standard Price |
| Labor Rate | (Actual Rate - Standard Rate) x Actual Hours |
| Labor Efficiency | (Actual Hours - Standard Hours) x Standard Rate |
| Overhead Volume | (Actual Volume - Budgeted Volume) x Rate |

### Favorable vs Unfavorable
- **Favorable**: Actual cost < Standard cost (positive variance)
- **Unfavorable**: Actual cost > Standard cost (negative variance)

## Cost Adjustments

Workflow for changing item costs:
1. Create adjustment with reason and new cost
2. Submit for approval
3. Approver reviews impact
4. Apply adjustment (updates item cost, revalues inventory)

Types: StandardCostUpdate, Revaluation, WriteOff, Correction

## BOM Cost Rollup

Calculate manufactured item cost from components:
```
Component A: 2 units @ $5.00 = $10.00
Component B: 1 unit @ $3.50 = $3.50
Component C: 4 units @ $1.25 = $5.00
─────────────────────────────────────
Material Cost:                 $18.50
Labor:                         $6.00
Overhead:                      $3.00
─────────────────────────────────────
Total Unit Cost:              $27.50
```

## Inventory Valuation

Summary report showing:
- Total inventory value by costing method
- Value by warehouse/location
- Value by product category
- COGS for the period
- Variance totals
