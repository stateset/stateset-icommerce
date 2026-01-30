import EventEmitter from 'events';

/**
 * Pattern 3: Multi-Agent Event Chain
 * 
 * Demonstrates a 4-agent order fulfillment chain where each agent:
 * 1. Processes an event
 * 2. Emits a new event to trigger the next agent
 * 3. Continues the chain until completion
 */

class OrderFulfillmentChain extends EventEmitter {
  constructor() {
    super();
    this.orderHistory = new Map();
    this.setupEventHandlers();
  }

  setupEventHandlers() {
    // Agent 1: Order Validation Agent
    this.on('order:received', async (order) => {
      console.log(`\n📦 VALIDATION AGENT: Processing order ${order.id}`);
      
      // Simulate validation
      await this.delay(500);
      
      // Check if valid
      const isValid = this.validateOrder(order);
      
      if (isValid) {
        console.log(`✓ Order ${order.id} is valid`);
        
        // Emit to trigger next agent
        this.emit('order:validated', {
          ...order,
          status: 'validated',
          validatedAt: new Date().toISOString()
        });
      } else {
        console.log(`✗ Order ${order.id} is invalid`);
        this.emit('order:rejected', { ...order, status: 'rejected' });
      }
    });

    // Agent 2: Inventory Agent
    this.on('order:validated', async (order) => {
      console.log(`\n🏭 INVENTORY AGENT: Reserving stock for order ${order.id}`);
      
      await this.delay(800);
      
      const reserved = await this.reserveInventory(order);
      
      if (reserved) {
        console.log(`✓ Reserved ${order.items.length} items for order ${order.id}`);
        
        // Update order state and emit to next agent
        this.emit('inventory:reserved', {
          ...order,
          status: 'inventory_reserved',
          reservedAt: new Date().toISOString()
        });
      } else {
        console.log(`✗ Out of stock for order ${order.id}`);
        this.emit('inventory:failed', { ...order, status: 'out_of_stock' });
      }
    });

    // Agent 3: Payment Agent
    this.on('inventory:reserved', async (order) => {
      console.log(`\n💳 PAYMENT AGENT: Processing payment for order ${order.id}`);
      
      await this.delay(1000);
      
      const paymentSuccess = await this.processPayment(order);
      
      if (paymentSuccess) {
        console.log(`✓ Payment processed for order ${order.id} ($${order.total})`);
        
        // Trigger fulfillment
        this.emit('payment:processed', {
          ...order,
          status: 'paid',
          paidAt: new Date().toISOString(),
          paymentId: `pay_${Date.now()}`
        });
      } else {
        console.log(`✗ Payment failed for order ${order.id}`);
        this.emit('payment:failed', { ...order, status: 'payment_failed' });
      }
    });

    // Agent 4: Fulfillment Agent
    this.on('payment:processed', async (order) => {
      console.log(`\n🚚 FULFILLMENT AGENT: Preparing shipment for order ${order.id}`);
      
      await this.delay(700);
      
      const shipped = await this.shipOrder(order);
      
      if (shipped) {
        console.log(`✓ Order ${order.id} shipped! Tracking: ${shipped.trackingNumber}`);
        
        // Mark as complete
        const completedOrder = {
          ...order,
          status: 'completed',
          shippedAt: new Date().toISOString(),
          trackingNumber: shipped.trackingNumber,
          estimatedDelivery: shipped.estimatedDelivery
        };
        
        this.orderHistory.set(order.id, completedOrder);
        this.emit('order:completed', completedOrder);
      }
    });

    // Error handlers
    this.on('order:rejected', (order) => {
      this.orderHistory.set(order.id, order);
      console.log(`❌ Order ${order.id} complete: ${order.status}`);
    });

    this.on('inventory:failed', (order) => {
      this.orderHistory.set(order.id, order);
      console.log(`❌ Order ${order.id} complete: ${order.status}`);
    });

    this.on('payment:failed', (order) => {
      this.orderHistory.set(order.id, order);
      console.log(`❌ Order ${order.id} complete: ${order.status}`);
    });
  }

  validateOrder(order) {
    // Simulate validation logic
    return order.customerId && order.items?.length > 0 && order.total > 0;
  }

  async reserveInventory(order) {
    // Simulate inventory check
    return true;
  }

  async processPayment(order) {
    // Simulate payment processing
    return Math.random() > 0.1; // 90% success rate
  }

  async shipOrder(order) {
    // Simulate shipping
    return {
      trackingNumber: `TRK${Date.now()}`,
      estimatedDelivery: new Date(Date.now() + 5 * 24 * 60 * 60 * 1000).toISOString()
    };
  }

  delay(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
  }

  start(order) {
    console.log(`\n========================================`);
    console.log(`Starting Order Fulfillment Chain`);
    console.log(`========================================`);
    console.log(`Order ID: ${order.id}`);
    console.log(`Customer: ${order.customerId}`);
    console.log(`Items: ${order.items.length}`);
    console.log(`Total: $${order.total}`);
    
    this.emit('order:received', order);
  }

  getOrderHistory(orderId) {
    return this.orderHistory.get(orderId);
  }
}

// ============= DEMO =============

async function runEventChainDemo() {
  const chain = new OrderFulfillmentChain();

  // Create sample orders
  const orders = [
    {
      id: 'ORD-001',
      customerId: 'CUST-001',
      items: ['laptop', 'mouse'],
      total: 1200.00
    },
    {
      id: 'ORD-002',
      customerId: 'CUST-002',
      items: ['headphones', 'keyboard'],
      total: 350.00
    }
  ];

  // Start both orders simultaneously (parallel chains)
  console.log('\n🚀 Starting PARALLEL order fulfillment chains...\n');
  
  orders.forEach(order => chain.start(order));

  // Wait for completion
  await new Promise(resolve => setTimeout(resolve, 5000));

  console.log('\n========================================');
  console.log('Chain Execution Complete');
  console.log('========================================');
  
  // Show results
  console.log('\n📊 Order Summary:');
  orders.forEach(order => {
    const result = chain.getOrderHistory(order.id);
    console.log(`  ${order.id}: ${result?.status || 'pending'}`);
  });
}

// Run the demo
runEventChainDemo().catch(console.error);