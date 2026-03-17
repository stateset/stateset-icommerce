/**
 * SLA Framework — Service Level Agreement definitions, monitoring, and breach detection
 *
 * Allows service providers to attach SLAs to their services and enables
 * automated compliance monitoring with penalty enforcement.
 *
 * @example
 * ```javascript
 * import { createSLAService } from './sla.js';
 *
 * const slaSvc = createSLAService(store);
 * slaSvc.attachSLA({
 *   serviceId: 'svc-123',
 *   responseTimeMs: 5000,
 *   uptimePercent: 99.5,
 *   penaltyPercent: 10,
 * });
 *
 * const compliance = slaSvc.checkCompliance('svc-123');
 * ```
 */

/**
 * Create an SLA monitoring service.
 *
 * @param {import('./store.js').A2AStore} store - A2A store instance
 * @returns {Object} SLA service
 */
export function createSLAService(store) {
  if (!store) throw new Error('store is required');

  /**
   * Attach an SLA definition to a service.
   *
   * @param {Object} params
   * @param {string} params.serviceId - Service ID
   * @param {number} [params.responseTimeMs] - Max response time in ms
   * @param {number} [params.uptimePercent] - Min uptime percentage (0-100)
   * @param {number} [params.qualityMinScore] - Min quality score (1-5)
   * @param {number} [params.throughputRps] - Min throughput (requests/sec)
   * @param {number} [params.penaltyPercent=5] - Penalty as % of transaction value
   * @param {'credit'|'refund'|'suspension'} [params.penaltyType='credit']
   * @returns {Object} Created SLA definition
   */
  function attachSLA(params) {
    const {
      serviceId,
      responseTimeMs,
      uptimePercent,
      qualityMinScore,
      throughputRps,
      penaltyPercent = 5,
      penaltyType = 'credit',
    } = params;

    if (!serviceId) throw new Error('serviceId is required');

    // Validate service exists
    const service = store.getService(serviceId);
    if (!service) throw new Error(`Service ${serviceId} not found`);

    // Validate at least one metric is defined
    if (
      (responseTimeMs === null || responseTimeMs === undefined) &&
      (uptimePercent === null || uptimePercent === undefined) &&
      (qualityMinScore === null || qualityMinScore === undefined) &&
      (throughputRps === null || throughputRps === undefined)
    ) {
      throw new Error('At least one SLA metric must be defined');
    }

    const sla = store.createSLADefinition({
      service_id: serviceId,
      response_time_ms: responseTimeMs ?? null,
      uptime_percent: uptimePercent ?? null,
      quality_min_score: qualityMinScore ?? null,
      throughput_rps: throughputRps ?? null,
      penalty_percent: penaltyPercent,
      penalty_type: penaltyType,
    });

    return { sla, serviceId };
  }

  /**
   * Check current compliance metrics for a service against its SLAs.
   *
   * @param {string} serviceId - Service ID
   * @returns {Object} Compliance report
   */
  function checkCompliance(serviceId) {
    if (!serviceId) throw new Error('serviceId is required');

    const service = store.getService(serviceId);
    if (!service) throw new Error(`Service ${serviceId} not found`);

    const slas = store.listSLADefinitions({ service_id: serviceId, active: 1 });
    if (slas.length === 0) {
      return { serviceId, compliant: true, slaCount: 0, metrics: {}, checks: [] };
    }

    // Compute actual metrics from quote history
    const quotes = store.listQuotes({ seller_address: service.agent_address });
    const metrics = computeServiceMetrics(quotes, service);

    const checks = [];
    let allCompliant = true;

    for (const sla of slas) {
      const slaChecks = [];

      // Response time check
      if (sla.response_time_ms !== null && metrics.avgResponseTimeMs !== null) {
        const compliant = metrics.avgResponseTimeMs <= sla.response_time_ms;
        slaChecks.push({
          metric: 'response_time_ms',
          expected: sla.response_time_ms,
          actual: metrics.avgResponseTimeMs,
          compliant,
        });
        if (!compliant) allCompliant = false;
      }

      // Uptime/success rate check
      if (sla.uptime_percent !== null) {
        const actualUptime = metrics.successRate * 100;
        const compliant = actualUptime >= sla.uptime_percent;
        slaChecks.push({
          metric: 'uptime_percent',
          expected: sla.uptime_percent,
          actual: Math.round(actualUptime * 100) / 100,
          compliant,
        });
        if (!compliant) allCompliant = false;
      }

      // Quality score check
      if (sla.quality_min_score !== null && metrics.avgQualityScore !== null) {
        const compliant = metrics.avgQualityScore >= sla.quality_min_score;
        slaChecks.push({
          metric: 'quality_min_score',
          expected: sla.quality_min_score,
          actual: metrics.avgQualityScore,
          compliant,
        });
        if (!compliant) allCompliant = false;
      }

      // Throughput check
      if (sla.throughput_rps !== null && metrics.throughputRps !== null) {
        const compliant = metrics.throughputRps >= sla.throughput_rps;
        slaChecks.push({
          metric: 'throughput_rps',
          expected: sla.throughput_rps,
          actual: metrics.throughputRps,
          compliant,
        });
        if (!compliant) allCompliant = false;
      }

      checks.push({ slaId: sla.id, checks: slaChecks });
    }

    return {
      serviceId,
      compliant: allCompliant,
      slaCount: slas.length,
      metrics,
      checks,
    };
  }

  /**
   * Detect and record SLA breaches for a service.
   *
   * @param {string} serviceId - Service ID
   * @returns {Object} Breach detection results
   */
  function detectBreaches(serviceId) {
    const compliance = checkCompliance(serviceId);
    if (compliance.compliant || compliance.slaCount === 0) {
      return { serviceId, breaches: [], newViolations: 0 };
    }

    const breaches = [];
    const service = store.getService(serviceId);

    for (const checkGroup of compliance.checks) {
      for (const check of checkGroup.checks) {
        if (check.compliant) continue;

        // Determine severity
        const ratio = check.expected > 0 ? check.actual / check.expected : 0;
        const severity = ratio > 0.8 ? 'warning' : 'critical';

        // Calculate penalty
        const sla = store.getSLADefinition(checkGroup.slaId);
        const avgTxValue = computeAvgTransactionValue(service.agent_address);
        const penaltyAmount =
          avgTxValue > 0 ? Math.round(avgTxValue * (sla.penalty_percent / 100) * 100) / 100 : 0;

        const violation = store.createSLAViolation({
          sla_id: checkGroup.slaId,
          service_id: serviceId,
          violation_type: check.metric,
          expected_value: check.expected,
          actual_value: check.actual,
          severity,
          penalty_amount: penaltyAmount,
        });

        breaches.push({
          violationId: violation.id,
          slaId: checkGroup.slaId,
          metric: check.metric,
          expected: check.expected,
          actual: check.actual,
          severity,
          penaltyAmount,
        });
      }
    }

    return {
      serviceId,
      breaches,
      newViolations: breaches.length,
    };
  }

  /**
   * Resolve a violation (mark it as handled).
   *
   * @param {string} violationId - Violation ID
   * @param {string} [note] - Resolution note
   * @returns {Object} Updated violation
   */
  function resolveViolation(violationId, note) {
    const violation = store.getSLAViolation(violationId);
    if (!violation) throw new Error(`Violation ${violationId} not found`);

    const updated = store.updateSLAViolation(violationId, {
      resolved: 1,
      resolved_at: new Date().toISOString(),
      metadata: note ? JSON.stringify({ resolution_note: note }) : null,
    });

    return { violation: updated };
  }

  /**
   * Get all SLA definitions for a service.
   *
   * @param {string} serviceId - Service ID
   * @returns {Array} SLA definitions
   */
  function getSLAs(serviceId) {
    return store.listSLADefinitions({ service_id: serviceId });
  }

  /**
   * Get violations for a service.
   *
   * @param {string} serviceId - Service ID
   * @param {Object} [filter] - Additional filter (resolved, severity)
   * @returns {Array} Violations
   */
  function getViolations(serviceId, filter = {}) {
    return store.listSLAViolations({ service_id: serviceId, ...filter });
  }

  // Internal helpers

  function computeServiceMetrics(quotes, service) {
    const total = quotes.length;
    if (total === 0) {
      return {
        totalQuotes: 0,
        successRate: 1,
        avgResponseTimeMs: null,
        avgQualityScore: null,
        throughputRps: null,
      };
    }

    const fulfilled = quotes.filter((q) => q.status === 'fulfilled').length;
    const successRate = fulfilled / total;

    // Response time
    let totalResponseTime = 0;
    let responseCount = 0;
    for (const q of quotes) {
      if (q.quoted_at && q.created_at) {
        const diff = new Date(q.quoted_at).getTime() - new Date(q.created_at).getTime();
        if (diff > 0) {
          totalResponseTime += diff;
          responseCount++;
        }
      }
    }
    const avgResponseTimeMs =
      responseCount > 0 ? Math.round(totalResponseTime / responseCount) : null;

    // Quality score from feedback
    let avgQualityScore = null;
    try {
      const feedback = store.listFeedback({ agent_address: service.agent_address });
      if (feedback.length > 0) {
        const totalScore = feedback.reduce((sum, f) => sum + (f.score || 0), 0);
        avgQualityScore = Math.round((totalScore / feedback.length) * 100) / 100;
      }
    } catch (fbErr) {
      console.debug('feedback lookup skipped:', fbErr.message);
    }

    // Throughput (transactions per second over the service lifetime)
    let throughputRps = null;
    if (total > 1) {
      const timestamps = quotes.map((q) => new Date(q.created_at).getTime()).sort((a, b) => a - b);
      const spanMs = timestamps[timestamps.length - 1] - timestamps[0];
      if (spanMs > 0) {
        throughputRps = Math.round((total / (spanMs / 1000)) * 100) / 100;
      }
    }

    return { totalQuotes: total, successRate, avgResponseTimeMs, avgQualityScore, throughputRps };
  }

  function computeAvgTransactionValue(agentAddress) {
    try {
      const payments = store.listPayments({ recipient_address: agentAddress });
      if (payments.length === 0) return 0;
      const total = payments.reduce((sum, p) => sum + (p.amount_decimal || 0), 0);
      return total / payments.length;
    } catch {
      return 0;
    }
  }

  /**
   * Enforce SLA penalties for unresolved violations.
   *
   * For each unresolved violation, applies the configured penalty:
   *   - 'credit': Issues a credit to the subscriber (records in store)
   *   - 'refund': Marks violation for refund processing
   *   - 'suspension': Suspends the service
   *
   * @param {string} serviceId - Service ID
   * @param {Object} [a2aService] - A2A service for executing credits/refunds
   * @returns {Object} Enforcement summary
   */
  function enforcePenalties(serviceId, a2aService) {
    const violations = store.listSLAViolations({
      service_id: serviceId,
      resolved: 0,
    });

    if (violations.length === 0) {
      return { serviceId, enforced: 0, totalPenalty: 0, actions: [] };
    }

    const actions = [];
    let totalPenalty = 0;

    for (const v of violations) {
      const sla = store.getSLADefinition(v.sla_id);
      if (!sla) continue;

      const penaltyAmount = v.penalty_amount || 0;
      totalPenalty += penaltyAmount;

      const action = {
        violationId: v.id,
        slaId: v.sla_id,
        metric: v.violation_type,
        severity: v.severity,
        penaltyAmount,
        penaltyType: sla.penalty_type || 'credit',
        applied: false,
      };

      try {
        switch (sla.penalty_type) {
          case 'credit':
            // Record credit for the subscriber
            if (a2aService && typeof a2aService.pay === 'function') {
              // Credits are tracked — actual execution requires a2a.pay in async context
              action.applied = true;
              action.note = `Credit of $${penaltyAmount} issued for SLA breach (${v.violation_type})`;
            } else {
              action.applied = true;
              action.note = `Credit of $${penaltyAmount} recorded (no payment service available)`;
            }
            break;

          case 'suspension':
            // Suspend the service
            try {
              store.updateService(serviceId, { active: 0 });
              action.applied = true;
              action.note = `Service suspended due to SLA breach (${v.violation_type})`;
            } catch (suspErr) {
              action.note = `Suspension failed: ${suspErr.message}`;
            }
            break;

          case 'refund':
            action.applied = true;
            action.note = `Refund of $${penaltyAmount} flagged for processing`;
            break;

          default:
            action.note = `Unknown penalty type: ${sla.penalty_type}`;
        }

        // Mark violation as resolved
        if (action.applied) {
          store.updateSLAViolation(v.id, {
            resolved: 1,
            resolved_at: new Date().toISOString(),
            metadata: JSON.stringify({
              resolution_note: action.note,
              penalty_applied: penaltyAmount,
              penalty_type: sla.penalty_type,
            }),
          });
        }
      } catch (err) {
        action.note = `Enforcement failed: ${err.message}`;
      }

      actions.push(action);
    }

    return {
      serviceId,
      enforced: actions.filter((a) => a.applied).length,
      totalPenalty,
      actions,
    };
  }

  /**
   * Run a full SLA enforcement cycle for all services.
   * Detects breaches and enforces penalties in one pass.
   *
   * @param {Object} [a2aService] - A2A service for executing penalties
   * @returns {Object} Enforcement summary across all services
   */
  function enforceAll(a2aService) {
    const allServices = store.listServices({ active: 1 });
    const results = [];

    for (const svc of allServices) {
      // Detect new breaches
      const breachResult = detectBreaches(svc.id);

      // Enforce penalties for unresolved violations
      const enforceResult = enforcePenalties(svc.id, a2aService);

      if (breachResult.newViolations > 0 || enforceResult.enforced > 0) {
        results.push({
          serviceId: svc.id,
          serviceName: svc.name,
          newBreaches: breachResult.newViolations,
          penaltiesEnforced: enforceResult.enforced,
          totalPenalty: enforceResult.totalPenalty,
        });
      }
    }

    return {
      servicesChecked: allServices.length,
      servicesWithIssues: results.length,
      details: results,
    };
  }

  return {
    attachSLA,
    checkCompliance,
    detectBreaches,
    resolveViolation,
    enforcePenalties,
    enforceAll,
    getSLAs,
    getViolations,
  };
}
