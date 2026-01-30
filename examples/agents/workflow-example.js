/**
 * Example 2: State Machine Workflow - Agent-to-Agent Handoff
 * 
 * This demonstrates a complete order fulfillment workflow where
 * multiple agents collaborate through state transitions.
 */

const { WorkflowEngine } = require('../../cli/src/workflows/state-machine');
const { autonomous } = require('../../cli/src/agents')
const { createClient } = require('@libsql/client');
const fs = require('fs');
const path = require('path');

class OrderFulfillmentWorkflow {
  constructor() {
    // In-memory database for demo
    const dbPath = path.join(__dirname, 'orders.db');
    this.db = createClient({ url: `file:${dbPath}` });
    
    this.workflowEngine = new WorkflowEngine({
      db: this.db,
      eventBus: new (require('events').EventEmitter)()
    });
    
    this.orders = new Map();
    this.agents = {
      inventory: autonomous('inventory'),
      orders: autonomous('orders'),
      shipping: autonomous('shipping'),
      // Simplified agent responses for demo
      _inventory: {
        reserve: async (orderId) => {
          console.log('📦 Inventory Agent: Reserving items for order', orderId);
          return { success: true, reserved: 3 };
        },
        confirm: async (orderId) => {
          console.log('✅ Inventory Agent: Confirmed reservation for order', orderId);
          return { success: true };
        }
      },
      _orders: {
        updateStatus: async (orderId, status) => {
          console.log(`📋 Orders Agent: Order ${orderId} status → ${status}`);
          return true;
        }
      },
      _shipping: {
        createLabel: async (orderId) => {
          console.log('🚚 Shipping Agent: Created label for order', orderId);
          return { trackingNumber: 'TRK-' + Math.random().toString(36).substr(2, 9) };
        },
        updateStatus: async (orderId, status, trackingNumber) => {
          console.log(`🚚 Shipping Agent: Order ${orderId} ${status} (Tracking: ${trackingNumber})`);
          return true;
        }
      }
    };
    
    this.setupWorkflow();
  }

  setupWorkflow() {
    const fulfillmentWorkflow = {
      id: 'order-fulfillment',
      initialState: 'pending',
      states: [
        { name: 'pending' },
        { 
          name: 'processing',
          timeout: 300000, // 5 minutes
          timeoutTransition: 'failed',
          onEnter: async (instance) => {
            const result = await this.agents._inventory.reserve(instance.entityId);
            if (result.success) {
              await this.workflowEngine.transition(instance.id, 'awaiting_payment');
            }
          }
        },
        { 
          name: 'awaiting_payment',
          timeout: 3600000, // 1 hour
          timeoutTransition: 'cancelled'
        },
        { 
          name: 'paid',
          onEnter: async (instance) => {
            await this.agents._inventory.confirm(instance.entityId);
            await this.agents._orders.updateStatus(instance.entityId, 'processing');
            await this.workflowEngine.transition(instance.id, 'shipped');
          }
        },
        { 
          name: 'shipped',
          onEnter: async (instance) => {
            const { trackingNumber } = await this.agents._shipping.createLabel(instance.entityId);
            await this.agents._orders.updateStatus(instance.entityId, 'shipped');
            await this.workflowEngine.transition(instance.id, 'delivered');
          }
        },
        { 
          name: 'delivered',
          onEnter: async (instance) => {
            await this.agents._orders.updateStatus(instance.entityId, 'delivered');
            console.log('✅ Order fulfillment complete:', instance.entityId);
          }
        },
        { name: 'failed' },
        { name: 'cancelled' }
      ],
      transitions: [
        { name: 'process', from: 'pending', to: 'processing' },
        { name: 'payment_received', from: 'awaiting_payment', to: 'paid' },
        { name: 'deliver', from: 'shipped', to: 'delivered' },
        { name: 'cancel', from: ['pending', 'processing', 'awaiting_payment'], to: 'cancelled' },
        { name: 'fail', from: ['processing', 'awaiting_payment'], to: 'failed' }
      ]
    };

    this.workflowEngine.registerWorkflow(fulfillmentWorkflow);
  }

  async createOrder(customerId, items) {
    const orderId = 'ORD-' + Math.random().toString(36).substr(2, 9).toUpperCase();
    
    const order = {
      id: orderId,
      customerId,
      items,
      status: 'pending',
      createdAt: new Date(),
      total: items.reduce((sum, item) => sum + item.price * item.quantity, 0)
    };
    
    this.orders.set(orderId, order);
    
    // Create workflow instance
    const instance = await this.workflowEngine.createInstance(
      'order-fulfillment',
      orderId,
      { customerId, itemCount: items.length }
    );
    
    console.log(`🛒 Order created: ${orderId} (${items.length} items, $${order.total})`);
    
    return { order, instance };
  }

  async processPayment(orderId) {
    const instance = await this.workflowEngine.getInstanceByEntity('order-fulfillment', orderId);
    if (instance.currentState === 'awaiting_payment') {
      console.log('💳 Payment received for order:', orderId);
      await this.workflowEngine.transition(instance.id, 'payment_received');
      return true;
    }
    return false;
  }

  async getStatus(orderId) {
    const order = this.orders.get(orderId);
    const instance = await this.workflowEngine.getInstanceByEntity('order-fulfillment', orderId);
    return {
      order,
      workflowStatus: instance?.currentState || 'not_found'
    };
  }
}

async function runWorkflowExample() {
  console.log('\n=== Multi-Agent Workflow Example ===\n');
  
  const workflow = new OrderFulfillmentWorkflow();
  
  // Create an order
  const { order } = await workflow.createOrder('CUST-001', [
    { sku: 'PROD-001', name: 'Widget', price: 29.99, quantity: 2 },
    { sku: 'PROD-002', name: 'Gadget', price: 49.99, quantity: 1 }
  ]);
  
  console.log('\n--- Workflow Auto-Progressing ---\n');
  
  // Wait a bit for workflow to progress
  await new Promise(resolve => setTimeout(resolve, 1000));
  
  // Simulate payment
  await workflow.processPayment(order.id);
  
  // Wait for automatic transitions
  await new Promise(resolve => setTimeout(resolve, 2000));
  
  // Check final status
  const status = await workflow.getStatus(order.id);
  console.log(`\n📊 Final Order Status: ${status.workflowStatus}\n`);
}

if (require.main === module) {
  runWorkflowExample().catch(console.error);
}

module.exports = { OrderFulfillmentWorkflow };