#!/usr/bin/env node
/**
 * StateSet iCommerce - Manufacturing Example
 *
 * This example demonstrates manufacturing capabilities:
 * - Bill of Materials (BOM) management
 * - Work order creation and tracking
 * - Production workflow
 * - Component management
 *
 * Run with: node 08_manufacturing.js
 */

const { Commerce } = require('@stateset/embedded');

async function main() {
  console.log('=== StateSet iCommerce - Manufacturing ===\n');

  const commerce = new Commerce(':memory:');

  // ============================================
  // Setup: Create Products for Manufacturing
  // ============================================

  console.log('[Setup] Creating products...');

  // Finished product
  const customPC = await commerce.products.create({
    name: 'Custom Gaming PC',
    description: 'High-performance custom-built gaming PC',
    variants: [{ sku: 'PC-GAMING-001', name: 'Standard Config', price: 1999.99 }]
  });
  console.log(`    Created: ${customPC.name} (${customPC.id})`);

  // Another finished product
  const workstation = await commerce.products.create({
    name: 'Pro Workstation',
    description: 'Professional workstation for content creators',
    variants: [{ sku: 'WS-PRO-001', name: 'Standard Config', price: 2999.99 }]
  });
  console.log(`    Created: ${workstation.name} (${workstation.id})`);

  // Create inventory for components
  const components = [
    { sku: 'CASE-001', name: 'Tower Case', initialQuantity: 50, reorderPoint: 10 },
    { sku: 'MB-001', name: 'Gaming Motherboard', initialQuantity: 30, reorderPoint: 8 },
    { sku: 'CPU-001', name: 'High-End CPU', initialQuantity: 25, reorderPoint: 5 },
    { sku: 'GPU-001', name: 'Gaming GPU', initialQuantity: 20, reorderPoint: 5 },
    { sku: 'RAM-001', name: '32GB DDR5 RAM', initialQuantity: 100, reorderPoint: 20 },
    { sku: 'SSD-001', name: '1TB NVMe SSD', initialQuantity: 80, reorderPoint: 15 },
    { sku: 'PSU-001', name: '850W Power Supply', initialQuantity: 40, reorderPoint: 10 },
    { sku: 'COOL-001', name: 'AIO Liquid Cooler', initialQuantity: 35, reorderPoint: 8 },
    { sku: 'FAN-001', name: 'RGB Case Fan', initialQuantity: 200, reorderPoint: 50 }
  ];

  for (const comp of components) {
    await commerce.inventory.createItem(comp);
  }
  console.log(`    Created ${components.length} component inventory items\n`);

  // ============================================
  // 1. Create Bill of Materials (BOM)
  // ============================================

  console.log('[1] Creating Bill of Materials...');

  // BOM for Gaming PC
  const gamingPcBom = await commerce.bom.create({
    name: 'Gaming PC BOM',
    productId: customPC.id,
    description: 'Bill of materials for standard gaming PC configuration',
    revision: '1.0'
  });

  console.log(`    Created BOM: ${gamingPcBom.name}`);
  console.log(`      BOM Number: ${gamingPcBom.bomNumber}`);
  console.log(`      Product ID: ${gamingPcBom.productId}`);
  console.log(`      Revision: ${gamingPcBom.revision}`);
  console.log(`      Status: ${gamingPcBom.status}\n`);

  // BOM for Workstation
  const workstationBom = await commerce.bom.create({
    name: 'Workstation BOM',
    productId: workstation.id,
    description: 'Bill of materials for professional workstation',
    revision: '1.0'
  });
  console.log(`    Created BOM: ${workstationBom.name}\n`);

  // ============================================
  // 2. Add Components to BOM
  // ============================================

  console.log('[2] Adding components to Gaming PC BOM...');

  const gamingPcComponents = [
    { componentSku: 'CASE-001', name: 'Tower Case', quantity: 1, unitOfMeasure: 'each' },
    { componentSku: 'MB-001', name: 'Gaming Motherboard', quantity: 1, unitOfMeasure: 'each' },
    { componentSku: 'CPU-001', name: 'High-End CPU', quantity: 1, unitOfMeasure: 'each' },
    { componentSku: 'GPU-001', name: 'Gaming GPU', quantity: 1, unitOfMeasure: 'each' },
    { componentSku: 'RAM-001', name: '32GB DDR5 RAM', quantity: 2, unitOfMeasure: 'each' }, // 64GB total
    { componentSku: 'SSD-001', name: '1TB NVMe SSD', quantity: 2, unitOfMeasure: 'each' },  // 2TB total
    { componentSku: 'PSU-001', name: '850W Power Supply', quantity: 1, unitOfMeasure: 'each' },
    { componentSku: 'COOL-001', name: 'AIO Liquid Cooler', quantity: 1, unitOfMeasure: 'each' },
    { componentSku: 'FAN-001', name: 'RGB Case Fan', quantity: 6, unitOfMeasure: 'each' }
  ];

  for (const comp of gamingPcComponents) {
    const added = await commerce.bom.addComponent(gamingPcBom.id, comp);
    console.log(`    Added: ${added.name} x${added.quantity} (${added.unitOfMeasure})`);
  }
  console.log('');

  // Add components to workstation BOM
  console.log('    Adding components to Workstation BOM...');
  const wsComponents = [
    { componentSku: 'CASE-001', name: 'Tower Case', quantity: 1 },
    { componentSku: 'MB-001', name: 'Gaming Motherboard', quantity: 1 },
    { componentSku: 'CPU-001', name: 'High-End CPU', quantity: 2 }, // Dual CPU
    { componentSku: 'RAM-001', name: '32GB DDR5 RAM', quantity: 4 }, // 128GB
    { componentSku: 'SSD-001', name: '1TB NVMe SSD', quantity: 4 },  // 4TB
    { componentSku: 'PSU-001', name: '850W Power Supply', quantity: 2 } // Redundant PSU
  ];

  for (const comp of wsComponents) {
    await commerce.bom.addComponent(workstationBom.id, comp);
  }
  console.log(`    Added ${wsComponents.length} components to Workstation BOM\n`);

  // ============================================
  // 3. Get BOM Components
  // ============================================

  console.log('[3] Retrieving BOM components...');

  const retrievedComponents = await commerce.bom.getComponents(gamingPcBom.id);
  console.log(`    Gaming PC BOM has ${retrievedComponents.length} components:`);
  for (const comp of retrievedComponents) {
    console.log(`      - ${comp.componentSku}: ${comp.name} x${comp.quantity}`);
  }
  console.log('');

  // ============================================
  // 4. Activate BOM
  // ============================================

  console.log('[4] Activating BOMs...');

  const activatedBom = await commerce.bom.activate(gamingPcBom.id);
  console.log(`    Activated: ${activatedBom.name}`);
  console.log(`      Status: ${activatedBom.status}`);

  await commerce.bom.activate(workstationBom.id);
  console.log(`    Activated: ${workstationBom.name}\n`);

  // ============================================
  // 5. Create Work Orders
  // ============================================

  console.log('[5] Creating work orders...');

  // Work order for 5 gaming PCs
  const wo1 = await commerce.workOrders.create({
    productId: customPC.id,
    bomId: gamingPcBom.id,
    quantityToBuild: 5,
    priority: 'high',
    notes: 'Rush order for customer pre-orders'
  });

  console.log(`    Created Work Order: ${wo1.workOrderNumber}`);
  console.log(`      Product: ${customPC.name}`);
  console.log(`      Quantity: ${wo1.quantityToBuild}`);
  console.log(`      Priority: ${wo1.priority}`);
  console.log(`      Status: ${wo1.status}`);

  // Work order for 3 workstations
  const wo2 = await commerce.workOrders.create({
    productId: workstation.id,
    bomId: workstationBom.id,
    quantityToBuild: 3,
    priority: 'medium',
    notes: 'Standard production run'
  });
  console.log(`    Created Work Order: ${wo2.workOrderNumber} (${wo2.quantityToBuild} workstations)\n`);

  // Work order without BOM
  const wo3 = await commerce.workOrders.create({
    productId: customPC.id,
    quantityToBuild: 2,
    priority: 'low',
    notes: 'Custom configuration - no standard BOM'
  });
  console.log(`    Created Work Order: ${wo3.workOrderNumber} (no BOM, custom build)\n`);

  // ============================================
  // 6. Start Work Orders
  // ============================================

  console.log('[6] Starting work orders...');

  const startedWo1 = await commerce.workOrders.start(wo1.id);
  console.log(`    Started: ${startedWo1.workOrderNumber}`);
  console.log(`      Status: ${startedWo1.status}`);

  const startedWo2 = await commerce.workOrders.start(wo2.id);
  console.log(`    Started: ${startedWo2.workOrderNumber}`);
  console.log(`      Status: ${startedWo2.status}\n`);

  // ============================================
  // 7. Record Progress / Complete Work Orders
  // ============================================

  console.log('[7] Recording production progress...');

  // Complete 3 of 5 gaming PCs
  let updatedWo1 = await commerce.workOrders.complete(wo1.id, 3);
  console.log(`    ${updatedWo1.workOrderNumber}: Completed ${updatedWo1.quantityCompleted}/${updatedWo1.quantityToBuild}`);
  console.log(`      Status: ${updatedWo1.status}`);

  // Complete remaining 2
  updatedWo1 = await commerce.workOrders.complete(wo1.id, 2);
  console.log(`    ${updatedWo1.workOrderNumber}: Completed ${updatedWo1.quantityCompleted}/${updatedWo1.quantityToBuild}`);
  console.log(`      Status: ${updatedWo1.status}`);

  // Complete all workstations at once
  const completedWo2 = await commerce.workOrders.complete(wo2.id, 3);
  console.log(`    ${completedWo2.workOrderNumber}: Completed ${completedWo2.quantityCompleted}/${completedWo2.quantityToBuild}`);
  console.log(`      Status: ${completedWo2.status}\n`);

  // ============================================
  // 8. Cancel Work Order
  // ============================================

  console.log('[8] Cancelling work order...');

  const cancelledWo = await commerce.workOrders.cancel(wo3.id);
  console.log(`    Cancelled: ${cancelledWo.workOrderNumber}`);
  console.log(`      Status: ${cancelledWo.status}\n`);

  // ============================================
  // 9. List and Query BOMs
  // ============================================

  console.log('[9] Querying BOMs...');

  // List all BOMs
  const allBoms = await commerce.bom.list();
  console.log(`    Total BOMs: ${allBoms.length}`);

  // Get BOM by ID
  const retrievedBom = await commerce.bom.get(gamingPcBom.id);
  console.log(`    Retrieved: ${retrievedBom.name} (${retrievedBom.bomNumber})`);

  // Count BOMs
  const bomCount = await commerce.bom.count();
  console.log(`    BOM count: ${bomCount}\n`);

  // ============================================
  // 10. List and Query Work Orders
  // ============================================

  console.log('[10] Querying work orders...');

  // List all work orders
  const allWorkOrders = await commerce.workOrders.list();
  console.log(`    Total work orders: ${allWorkOrders.length}`);

  console.log('    Work order summary:');
  for (const wo of allWorkOrders) {
    console.log(`      ${wo.workOrderNumber}:`);
    console.log(`        Status: ${wo.status}`);
    console.log(`        Priority: ${wo.priority}`);
    console.log(`        Progress: ${wo.quantityCompleted}/${wo.quantityToBuild}`);
  }

  // Get specific work order
  const retrievedWo = await commerce.workOrders.get(wo1.id);
  console.log(`\n    Retrieved: ${retrievedWo.workOrderNumber}`);
  console.log(`      Version: ${retrievedWo.version}`);
  console.log(`      Created: ${retrievedWo.createdAt}`);
  console.log(`      Updated: ${retrievedWo.updatedAt}`);

  // Count work orders
  const woCount = await commerce.workOrders.count();
  console.log(`    Work order count: ${woCount}\n`);

  // ============================================
  // 11. Production Summary
  // ============================================

  console.log('[11] Production summary...');

  // Calculate total production
  const completedOrders = allWorkOrders.filter(wo => wo.status === 'completed');
  const inProgressOrders = allWorkOrders.filter(wo => wo.status === 'in_progress');
  const cancelledOrders = allWorkOrders.filter(wo => wo.status === 'cancelled');

  const totalBuilt = completedOrders.reduce((sum, wo) => sum + wo.quantityCompleted, 0);

  console.log('    Production Statistics:');
  console.log(`      Completed Work Orders: ${completedOrders.length}`);
  console.log(`      In Progress: ${inProgressOrders.length}`);
  console.log(`      Cancelled: ${cancelledOrders.length}`);
  console.log(`      Total Units Built: ${totalBuilt}`);

  // Check inventory levels after production (would need to manually adjust in real scenario)
  console.log('\n    Component inventory check:');
  for (const sku of ['CASE-001', 'GPU-001', 'CPU-001', 'RAM-001']) {
    const stock = await commerce.inventory.getStock(sku);
    console.log(`      ${stock.name}: ${stock.totalAvailable} available`);
  }

  console.log('\n=== Manufacturing Example Complete ===');
}

main().catch(console.error);
