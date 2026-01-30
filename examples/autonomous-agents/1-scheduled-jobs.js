/**
 * Example 1: Scheduled Jobs Agent Coordination
 * 
 * Two agents running autonomously on a schedule, coordinating through shared database state.
 * 
 * Pattern:
 * 1. Inventory monitoring agent runs every hour, detects low stock
 * 2. Purchasing agent runs every 30 minutes, reacts to low stock alerts
 * 3. Both agents coordinate through the shared inventory database
 */

import { AutonomousEngine } from '../../cli/src/autonomous/engine.js';
import { Scheduler } from '../../cli/src/workflows/scheduler.js';
import { ICommerceClient } from '../../cli/src/client/client.js';

/**
 * Define the two scheduled jobs for autonomous agent coordination
 */
const ScheduledAgentJobs = {
  // Agent 1: Inventory Monitor - Proactive monitoring
  inventoryMonitor: {
    name: 'Inventory Stock Monitor',
    schedule: '0 * * * *', // Run every hour
    description: 'Checks inventory levels and creates low stock alerts',
    action: {
      agent: 'inventory',
      request: 'Check all products and identify any items below their reorder threshold. If found, create a low_stock_event in the database with SKU, current quantity, and reorder point.',
      context: {
        action: 'create_alert',
        alertType: 'low_stock'
      }
    }
  },

  // Agent 2: Purchasing Bot - Reactive purchasing
  purchasingBot: {
    name: 'Automated Purchasing Agent',
    schedule: '*/30 * * * *', // Run every 30 minutes
    description: 'Reviews low stock alerts and creates purchase orders',
    action: {
      agent: 'purchasing',
      request: 'Review all unresolved low_stock_events in the database. For each alert, calculate the optimal order quantity based on historical demand patterns and create a POApprovalRequest in the approvals queue.',
      context: {
        action: 'create_purchase_order',
       审批流程: 'auto_approve_under_500'
      }
    }
  },

  // Agent 3: Reports Agent - Coordination summary
  reportingBot: {
    name: 'Coordination Report Generator',
    schedule: '0 9 * * *', // Run daily at 9 AM
    description: 'Generates summary of agent coordination',
    action: {
      agent: 'analytics',
      request: 'Analyze agent coordination effectiveness: count low stock alerts created by inventory_monitor, count POs created by purchasing_bot, calculate fulfillment rate, and generate daily coordination summary report.',
      context: {
        action: 'coordination_report',
        agentsInvolved: ['inventory', 'purchasing', 'analytics']
      }
    }
  }
};

/**
 * Run the autonomous scheduled agents
 */
async function runScheduledAgents() {
  console.log('🤖 Starting Autonomous Agent Coordination via Scheduled Jobs\n');

  // Initialize the autonomous engine
  const engine = new AutonomousEngine({
    scheduler: {
      jobs: ScheduledAgentJobs
    },
    storage: {
      type: 'sqlite',
      path: './data/agents.db'
    }
  });

  // Listen to events to visualize coordination
  engine.on('scheduler:job:started', (job) => {
    console.log(`\n📅 ${new Date().toISOString()}`);
    console.log(`⏰ Scheduled Job Started: ${job.name}`);
    console.log(`   Agent: ${job.action.agent}`);
    console.log(`   Request: ${job.action.request.substring(0, 100)}...`);
  });

  engine.on('scheduler:job:completed', (job, result) => {
    console.log(`   ✅ Job Completed`);
    console.log(`   Result: ${JSON.stringify(result, null, 2).substring(0, 200)}...`);
  });

  // Watch for cross-agent coordination events
  engine.on('scheduler:job:completed', (job, result) => {
    if (job.name === 'Inventory Stock Monitor' && result.alertsCreated) {
      console.log(`   📊 Created ${result.alertsCreated} low stock alerts`);
      console.log(`   👉 These will be picked up by Purchasing Bot on next run\n`);
    }
    
    if (job.name === 'Automated Purchasing Agent' && result.purchaseOrdersCreated) {
      console.log(`   📦 Created ${result.purchaseOrdersCreated} purchase orders`);
      console.log(`   👉 These will be resolved by inventory on next stock recepit\n`);
    }
  });

  // Start the engine
  await engine.start();
  console.log('\n✨ Autonomous agents are now running!');
  console.log('   - Inventory Monitor: Every hour');
  console.log('   - Purchasing Bot: Every 30 minutes');
  console.log('   - Reporting Bot: Daily at 9 AM');
  console.log('\nCtrl+C to stop\n');
}

/**
 * Alternative: Direct scheduler setup without full autonomous engine
 */
async function runSimpleScheduledExample() {
  const Scheduler = await import('../../cli/src/workflows/scheduler.js');
  
  const scheduler = Scheduler.setupScheduler({
    jobs: ScheduledAgentJobs
  });

  // Listen for job events
  scheduler.on('job:started', (job) => {
    console.log(`\n⏰ [${new Date().toLocaleTimeString()}] ${job.name}`);
    console.log(`   Agent: ${job.action.agent}`);
  });

  scheduler.on('job:completed', (job, result) => {
    console.log(`   ✅ Completed: ${result.summary || 'OK'}`);
  });

  // Start the scheduler
  await scheduler.start();
  console.log('✨ Scheduled agents running...');
}

// Run the example
runScheduledAgents()
  .then(() => {
    console.log('All autonomous agents started successfully');
  })
  .catch(error => {
    console.error('Failed to start agents:', error);
    process.exit(1);
  });

export { ScheduledAgentJobs, runScheduledAgents };