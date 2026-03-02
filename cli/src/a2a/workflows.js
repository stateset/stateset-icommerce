/**
 * Workflow Orchestration — DAG-based multi-agent workflow execution
 *
 * Enables composing multi-agent workflows (A→B→C pipelines) with
 * dependency management, parallel fan-out, cost tracking, and
 * checkpoint-based resumability.
 *
 * @example
 * ```javascript
 * import { createWorkflowService } from './workflows.js';
 *
 * const wfSvc = createWorkflowService(store, a2aService);
 * const wf = wfSvc.createWorkflow({
 *   name: 'data-pipeline',
 *   steps: [
 *     { name: 'fetch', type: 'quote_request', agentAddress: '0xFetcher', params: { query: 'stock data' } },
 *     { name: 'analyze', type: 'quote_request', agentAddress: '0xAnalyzer', dependsOn: ['fetch'] },
 *     { name: 'report', type: 'transform', dependsOn: ['analyze'] },
 *   ],
 * });
 * await wfSvc.executeWorkflow(wf.workflow.id);
 * ```
 */

/**
 * Valid step types for workflow steps.
 */
const STEP_TYPES = new Set(['quote_request', 'payment', 'condition_check', 'transform']);

/**
 * Create a workflow orchestration service.
 *
 * @param {import('./store.js').A2AStore} store - A2A store instance
 * @param {Object} [a2aService] - A2A service instance (for quote/payment operations)
 * @returns {Object} Workflow service
 */
export function createWorkflowService(store, a2aService) {
  if (!store) throw new Error('store is required');

  /**
   * Validate that a step dependency graph is a valid DAG (no cycles).
   * Uses Kahn's algorithm for topological sort.
   *
   * @param {Array} steps - Step definitions
   * @returns {{ valid: boolean, order: string[], error?: string }}
   */
  function validateDAG(steps) {
    if (!Array.isArray(steps) || steps.length === 0) {
      return { valid: false, order: [], error: 'Steps array is required and must not be empty' };
    }

    const names = new Set(steps.map((s) => s.name));

    // Check for duplicate names
    if (names.size !== steps.length) {
      return { valid: false, order: [], error: 'Duplicate step names found' };
    }

    // Validate step types
    for (const step of steps) {
      if (!step.name) return { valid: false, order: [], error: 'All steps must have a name' };
      if (step.type && !STEP_TYPES.has(step.type)) {
        return { valid: false, order: [], error: `Invalid step type: ${step.type}` };
      }
    }

    // Build adjacency list and in-degree map
    const inDegree = new Map();
    const adjacency = new Map();

    for (const step of steps) {
      inDegree.set(step.name, 0);
      adjacency.set(step.name, []);
    }

    for (const step of steps) {
      const deps = step.dependsOn || [];
      for (const dep of deps) {
        if (!names.has(dep)) {
          return {
            valid: false,
            order: [],
            error: `Step "${step.name}" depends on unknown step "${dep}"`,
          };
        }
        adjacency.get(dep).push(step.name);
        inDegree.set(step.name, inDegree.get(step.name) + 1);
      }
    }

    // Kahn's algorithm
    const queue = [];
    for (const [name, deg] of inDegree) {
      if (deg === 0) queue.push(name);
    }

    const order = [];
    while (queue.length > 0) {
      const node = queue.shift();
      order.push(node);
      for (const neighbor of adjacency.get(node)) {
        const newDeg = inDegree.get(neighbor) - 1;
        inDegree.set(neighbor, newDeg);
        if (newDeg === 0) queue.push(neighbor);
      }
    }

    if (order.length !== steps.length) {
      return { valid: false, order: [], error: 'Cycle detected in workflow dependency graph' };
    }

    return { valid: true, order };
  }

  /**
   * Create a workflow with validated DAG steps.
   *
   * @param {Object} params
   * @param {string} params.name - Workflow name
   * @param {Array} params.steps - Step definitions
   * @param {Object} [params.metadata] - Optional metadata
   * @returns {Object} Created workflow with steps
   */
  function createWorkflow(params) {
    const { name, steps, metadata } = params;

    if (!name) throw new Error('Workflow name is required');
    if (!steps || !Array.isArray(steps) || steps.length === 0) {
      throw new Error('Steps array is required and must not be empty');
    }

    // Validate DAG
    const dagResult = validateDAG(steps);
    if (!dagResult.valid) {
      throw new Error(`Invalid workflow DAG: ${dagResult.error}`);
    }

    // Create workflow record
    const workflow = store.createWorkflow({
      name,
      definition: JSON.stringify({ steps, executionOrder: dagResult.order }),
      metadata: metadata || null,
    });

    // Create step records
    const stepRecords = [];
    for (const step of steps) {
      const record = store.createWorkflowStep({
        workflow_id: workflow.id,
        step_name: step.name,
        step_type: step.type || 'quote_request',
        agent_address: step.agentAddress || null,
        params: step.params || null,
        depends_on: step.dependsOn || [],
      });
      stepRecords.push(record);
    }

    return {
      workflow: store.getWorkflow(workflow.id),
      steps: stepRecords,
      executionOrder: dagResult.order,
    };
  }

  /**
   * Execute a workflow, processing steps in topological order.
   *
   * @param {string} workflowId - Workflow ID
   * @param {Object} [context={}] - Shared context passed between steps
   * @returns {Object} Execution result
   */
  async function executeWorkflow(workflowId, context = {}) {
    const workflow = store.getWorkflow(workflowId);
    if (!workflow) throw new Error(`Workflow ${workflowId} not found`);
    if (workflow.status === 'completed') {
      return { workflowId, status: 'completed', message: 'Workflow already completed' };
    }
    if (workflow.status === 'paused') {
      return {
        workflowId,
        status: 'paused',
        message: 'Workflow is paused. Use resumeWorkflow() first.',
      };
    }

    // Start the workflow
    store.updateWorkflow(workflowId, {
      status: 'running',
      started_at: workflow.started_at || new Date().toISOString(),
    });

    const definition = JSON.parse(workflow.definition || '{}');
    const executionOrder = definition.executionOrder || [];
    const allSteps = store.listWorkflowSteps({ workflow_id: workflowId });
    const stepMap = new Map(allSteps.map((s) => [s.step_name, s]));
    const stepResults = { ...context };
    let totalCost = workflow.total_cost || 0;

    for (const stepName of executionOrder) {
      const step = stepMap.get(stepName);
      if (!step) continue;

      // Skip already completed steps (for resume)
      if (step.status === 'completed') {
        if (step.result) {
          try {
            stepResults[stepName] = JSON.parse(step.result);
          } catch {
            /* ignore */
          }
        }
        continue;
      }

      // Check if paused
      const currentWf = store.getWorkflow(workflowId);
      if (currentWf.status === 'paused') {
        return {
          workflowId,
          status: 'paused',
          completedSteps: countCompleted(allSteps),
          totalCost,
        };
      }

      // Update current step
      store.updateWorkflow(workflowId, { current_step: stepName });
      store.updateWorkflowStep(step.id, {
        status: 'running',
        started_at: new Date().toISOString(),
      });

      try {
        // Gather dependency results
        const deps = step.depends_on ? JSON.parse(step.depends_on) : [];
        const depResults = {};
        for (const dep of deps) {
          depResults[dep] = stepResults[dep] || null;
        }

        const stepParams = step.params ? JSON.parse(step.params) : {};
        const result = await executeStep(step, stepParams, depResults, stepResults);

        const cost = result.cost || 0;
        totalCost += cost;

        store.updateWorkflowStep(step.id, {
          status: 'completed',
          result: JSON.stringify(result),
          cost,
          completed_at: new Date().toISOString(),
        });

        store.updateWorkflow(workflowId, { total_cost: totalCost });
        stepResults[stepName] = result;
      } catch (err) {
        store.updateWorkflowStep(step.id, {
          status: 'failed',
          error: err.message,
        });

        store.updateWorkflow(workflowId, {
          status: 'failed',
          error: `Step "${stepName}" failed: ${err.message}`,
          current_step: stepName,
        });

        return {
          workflowId,
          status: 'failed',
          failedStep: stepName,
          error: err.message,
          totalCost,
          completedSteps: countCompleted(store.listWorkflowSteps({ workflow_id: workflowId })),
        };
      }
    }

    // All steps completed
    store.updateWorkflow(workflowId, {
      status: 'completed',
      total_cost: totalCost,
      completed_at: new Date().toISOString(),
      current_step: null,
    });

    return {
      workflowId,
      status: 'completed',
      totalCost,
      completedSteps: executionOrder.length,
      results: stepResults,
    };
  }

  /**
   * Execute a single workflow step.
   */
  async function executeStep(step, params, depResults, _allResults) {
    switch (step.step_type) {
      case 'quote_request': {
        if (!a2aService) {
          return {
            success: true,
            message: 'Quote request (no a2a service)',
            cost: 0,
            simulated: true,
          };
        }
        if (!step.agent_address) {
          return { success: false, error: 'No agent_address for quote_request step', cost: 0 };
        }
        const items = params.items || [
          { description: params.description || step.step_name, quantity: 1 },
        ];
        const quoteResult = await a2aService.requestQuote({
          seller: step.agent_address,
          items,
          message: params.message || `Workflow step: ${step.step_name}`,
          maxRounds: params.maxRounds || 1,
        });
        return {
          success: true,
          quoteId: quoteResult.quote?.id,
          total: quoteResult.quote?.total_decimal || 0,
          cost: quoteResult.quote?.total_decimal || 0,
        };
      }

      case 'payment': {
        if (!a2aService) {
          return { success: true, message: 'Payment (no a2a service)', cost: 0, simulated: true };
        }
        const amount = params.amount || 0;
        const to = step.agent_address || params.to;
        if (!to) return { success: false, error: 'No recipient for payment step', cost: 0 };
        const payResult = await a2aService.pay({
          to,
          amount,
          asset: params.asset || 'USDC',
          memo: params.memo || `Workflow payment: ${step.step_name}`,
        });
        return { success: true, paymentId: payResult.payment?.id, cost: amount };
      }

      case 'condition_check': {
        // Evaluate a condition against dependency results
        const checkFn = params.check || 'exists';
        const checkTarget = params.target;

        if (checkFn === 'exists') {
          const exists = checkTarget
            ? !!depResults[checkTarget]
            : Object.keys(depResults).length > 0;
          if (!exists)
            throw new Error(`Condition check failed: ${checkTarget || 'dependencies'} not found`);
          return { success: true, condition: checkFn, passed: true, cost: 0 };
        }

        if (checkFn === 'min_value') {
          const value = checkTarget ? depResults[checkTarget]?.total : null;
          if (value === null || value === undefined || value < (params.minValue || 0)) {
            throw new Error(
              `Condition check failed: value ${value} below minimum ${params.minValue}`,
            );
          }
          return { success: true, condition: checkFn, value, passed: true, cost: 0 };
        }

        return { success: true, condition: checkFn, passed: true, cost: 0 };
      }

      case 'transform': {
        // Transform takes dependency results and produces output
        const transformType = params.transformType || 'merge';

        if (transformType === 'merge') {
          return { success: true, merged: depResults, cost: 0 };
        }

        if (transformType === 'sum_costs') {
          let totalCost = 0;
          for (const result of Object.values(depResults)) {
            totalCost += result?.cost || result?.total || 0;
          }
          return { success: true, totalCost, cost: 0 };
        }

        if (transformType === 'aggregate') {
          return {
            success: true,
            aggregated: Object.entries(depResults).map(([step, result]) => ({
              step,
              success: result?.success ?? true,
              cost: result?.cost || 0,
            })),
            cost: 0,
          };
        }

        return { success: true, data: depResults, cost: 0 };
      }

      default:
        throw new Error(`Unknown step type: ${step.step_type}`);
    }
  }

  /**
   * Get workflow status with step details.
   *
   * @param {string} workflowId - Workflow ID
   * @returns {Object} Workflow status
   */
  function getWorkflowStatus(workflowId) {
    const workflow = store.getWorkflow(workflowId);
    if (!workflow) throw new Error(`Workflow ${workflowId} not found`);

    const steps = store.listWorkflowSteps({ workflow_id: workflowId });

    return {
      workflow: {
        id: workflow.id,
        name: workflow.name,
        status: workflow.status,
        totalCost: workflow.total_cost,
        currentStep: workflow.current_step,
        error: workflow.error,
        createdAt: workflow.created_at,
        startedAt: workflow.started_at,
        completedAt: workflow.completed_at,
      },
      steps: steps.map((s) => ({
        id: s.id,
        name: s.step_name,
        type: s.step_type,
        status: s.status,
        cost: s.cost,
        error: s.error,
        agentAddress: s.agent_address,
        startedAt: s.started_at,
        completedAt: s.completed_at,
      })),
      progress: {
        total: steps.length,
        completed: steps.filter((s) => s.status === 'completed').length,
        failed: steps.filter((s) => s.status === 'failed').length,
        pending: steps.filter((s) => s.status === 'pending').length,
        running: steps.filter((s) => s.status === 'running').length,
      },
    };
  }

  /**
   * Pause a running workflow.
   *
   * @param {string} workflowId - Workflow ID
   * @returns {Object} Updated workflow
   */
  function pauseWorkflow(workflowId) {
    const workflow = store.getWorkflow(workflowId);
    if (!workflow) throw new Error(`Workflow ${workflowId} not found`);
    if (workflow.status !== 'running' && workflow.status !== 'pending') {
      throw new Error(`Cannot pause workflow in "${workflow.status}" status`);
    }

    store.updateWorkflow(workflowId, { status: 'paused' });
    return { workflowId, status: 'paused' };
  }

  /**
   * Resume a paused workflow.
   *
   * @param {string} workflowId - Workflow ID
   * @returns {Object} Resume result
   */
  async function resumeWorkflow(workflowId) {
    const workflow = store.getWorkflow(workflowId);
    if (!workflow) throw new Error(`Workflow ${workflowId} not found`);
    if (workflow.status !== 'paused') {
      throw new Error(`Cannot resume workflow in "${workflow.status}" status`);
    }

    store.updateWorkflow(workflowId, { status: 'running' });
    return executeWorkflow(workflowId);
  }

  function countCompleted(steps) {
    return steps.filter((s) => s.status === 'completed').length;
  }

  return {
    createWorkflow,
    validateDAG,
    executeWorkflow,
    getWorkflowStatus,
    pauseWorkflow,
    resumeWorkflow,
  };
}
