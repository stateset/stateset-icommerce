export const ScheduledAgents = {
  /**
   * Example 1: Two independent agents running on schedules
   * These agents work autonomously but can coordinate through shared state
   */

  jobs: [
    {
      name: 'Inventory Monitor Agent',
      schedule: '*/5 * * * *', // Every 5 minutes
      description: 'Monitors stock levels and triggers replenishment when needed',
      action: {
        agent: 'inventory',
        request: 'Check all products and identify any items below their reorder point',
        onEvent: {
          event: 'low_stock_detected',
          action: {
            triggerAgent: 'suppliers',
            request: 'Create purchase order for {sku} - reorder quantity: {reorderQty}'
          }
        }
      }
    },
    {
      name: 'Order Fulfillment Agent',
      schedule: '*/1 * * * *', // Every minute
      description: 'Processes pending orders and moves them through fulfillment pipeline',
      action: {
        agent: 'orders',
        request: 'List all orders in "pending" status and process them for fulfillment',
        onEvent: {
          event: 'order_ready_for_shipment',
          action: {
            triggerAgent: 'shipping',
            request: 'Generate shipping label for order {orderId}'
          }
        }
      }
    }
  ],

  /**
   * How to run this example:
   *
   * 1. Create a workflow definition file: workflows/scheduled-agents.js
   * 2. Add jobs to your scheduler configuration
   * 3. Run: npx icommerce autonomous start
   */

  setup: {
    workflowFile: 'workflows/scheduled-agents.js',
    command: 'npx icommerce autonomous start',
    expectedBehavior: [
      'Inventory agent runs every 5 minutes',
      'Orders agent runs every 1 minute',
      'When inventory detects low stock, suppliers agent is triggered',
      'When orders are ready, shipping agent is triggered'
    ]
  }
};