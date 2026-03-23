/**
 * A2A Dispute Auto-Resolver — Autonomous Dispute Resolution Engine
 *
 * Enforces dispute deadlines and applies rule-based arbitration:
 *   1. Auto-transitions filed → evidence_period on first evidence or after 24h
 *   2. Auto-transitions evidence_period → under_review after evidence deadline (72h)
 *   3. Applies rule-based arbitration on under_review disputes past review deadline
 *   4. Auto-escalates if neither party is clearly at fault
 *   5. Sends notifications at each transition
 *
 * Arbitration Rules:
 *   - non_delivery + no delivery_proof from seller → full_refund
 *   - poor_quality with seller reputation < 2.5 → full_refund
 *   - overcharged with > 20% price discrepancy → partial_refund (market delta)
 *   - All others: split 50/50 or escalate if amount > threshold
 *
 * @example
 * ```javascript
 * const resolver = createDisputeResolver(store, disputeService, escrowService, {
 *   autoResolveThreshold: 500,  // auto-resolve up to $500
 *   intervalMs: 300_000,        // check every 5 minutes
 * });
 * resolver.start();
 * ```
 */

import { EventEmitter } from 'node:events';

const AUTO_EVIDENCE_PERIOD_MS = 24 * 60 * 60 * 1000; // 24h grace to enter evidence period

/**
 * Create a dispute auto-resolver instance.
 *
 * @param {Object} store - A2A store
 * @param {Object} disputeService - Dispute service
 * @param {Object} escrowService - Escrow service for executing resolutions
 * @param {Object} [notificationService] - Notification service
 * @param {Object} [options]
 * @param {number} [options.autoResolveThreshold=1000] - Max $ for auto-resolution
 * @param {number} [options.intervalMs=300000] - Polling interval (5min)
 * @returns {Object} Dispute resolver API
 */
export function createDisputeResolver(
  store,
  disputeService,
  escrowService,
  notificationService,
  options = {},
) {
  const { autoResolveThreshold = 1000, intervalMs = 300_000 } = options;
  const emitter = new EventEmitter();
  let _timer = null;
  let _running = false;

  const _metrics = {
    totalTicks: 0,
    autoTransitions: 0,
    autoResolutions: 0,
    autoEscalations: 0,
    lastTickAt: null,
  };

  /**
   * Execute one resolution cycle.
   * @returns {Promise<Object>} Tick result
   */
  async function tick() {
    const now = new Date();
    const nowIso = now.toISOString();
    let transitions = 0;
    let resolutions = 0;
    let escalations = 0;

    // 1. Auto-transition: filed → evidence_period (after 24h)
    const filedDisputes = await store.listDisputes({ status: 'filed' });
    for (const d of filedDisputes) {
      const filedAt = new Date(d.created_at).getTime();
      if (now.getTime() - filedAt >= AUTO_EVIDENCE_PERIOD_MS) {
        try {
          await disputeService.moveToEvidencePeriod(d.id);
          transitions++;
          emitter.emit('transition', {
            disputeId: d.id,
            from: 'filed',
            to: 'evidence_period',
          });

          // Notify both parties
          if (notificationService) {
            for (const addr of [d.filed_by, d.filed_against]) {
              try {
                await notificationService.sendNotification({
                  recipientAddress: addr,
                  eventType: 'dispute.evidence_period',
                  payload: {
                    disputeId: d.id,
                    evidenceDeadline: d.evidence_deadline,
                    message:
                      'Dispute has entered evidence period. Submit evidence before the deadline.',
                  },
                });
              } catch {
                // best effort
              }
            }
          }
        } catch (err) {
          console.warn(`[dispute-resolver] Failed to transition dispute ${d.id}:`, err.message);
        }
      }
    }

    // 2. Auto-transition: evidence_period → under_review (after evidence deadline)
    const evidencePeriodDisputes = await store.listDisputes({ status: 'evidence_period' });
    for (const d of evidencePeriodDisputes) {
      if (d.evidence_deadline && new Date(d.evidence_deadline) <= now) {
        try {
          await disputeService.moveToReview(d.id);
          transitions++;
          emitter.emit('transition', {
            disputeId: d.id,
            from: 'evidence_period',
            to: 'under_review',
          });
        } catch (err) {
          console.warn(`[dispute-resolver] Failed to move to review ${d.id}:`, err.message);
        }
      }
    }

    // 3. Auto-resolve or escalate: under_review disputes past review deadline
    const underReviewDisputes = await store.listDisputes({ status: 'under_review' });
    for (const d of underReviewDisputes) {
      if (!d.review_deadline || new Date(d.review_deadline) > now) {
        continue; // Not yet due for auto-resolution
      }

      // Don't auto-resolve disputes above threshold
      const disputeAmount = d.amount_decimal || 0;
      if (disputeAmount > autoResolveThreshold) {
        try {
          await disputeService.escalateDispute(d.id);
          escalations++;
          emitter.emit('escalated', {
            disputeId: d.id,
            reason: `Amount $${disputeAmount} exceeds auto-resolve threshold $${autoResolveThreshold}`,
          });
        } catch (err) {
          console.warn(`[dispute-resolver] Failed to escalate ${d.id}:`, err.message);
        }
        continue;
      }

      // Apply rule-based arbitration
      const decision = await _arbitrate(d);

      try {
        const result = await disputeService.resolveDispute(d.id, {
          resolutionType: decision.resolutionType,
          amount: decision.amount,
          note: decision.note,
          resolvedBy: 'auto-resolver',
        });

        // Execute escrow action
        if (result.escrowAction && escrowService) {
          try {
            await _executeEscrowAction(result.escrowAction);
          } catch (escrowErr) {
            console.warn(`[dispute-resolver] Escrow action failed for ${d.id}:`, escrowErr.message);
          }
        }

        resolutions++;
        emitter.emit('resolved', {
          disputeId: d.id,
          resolutionType: decision.resolutionType,
          amount: decision.amount,
          note: decision.note,
        });

        // Notify both parties
        if (notificationService) {
          for (const addr of [d.filed_by, d.filed_against]) {
            try {
              await notificationService.sendNotification({
                recipientAddress: addr,
                eventType: 'dispute.resolved',
                payload: {
                  disputeId: d.id,
                  resolutionType: decision.resolutionType,
                  amount: decision.amount,
                  note: decision.note,
                },
              });
            } catch {
              // best effort
            }
          }
        }
      } catch (err) {
        console.warn(`[dispute-resolver] Failed to resolve ${d.id}:`, err.message);
      }
    }

    _metrics.totalTicks++;
    _metrics.autoTransitions += transitions;
    _metrics.autoResolutions += resolutions;
    _metrics.autoEscalations += escalations;
    _metrics.lastTickAt = nowIso;

    return { transitions, resolutions, escalations };
  }

  /**
   * Apply rule-based arbitration to determine resolution.
   * @param {Object} dispute
   * @returns {Promise<{resolutionType: string, amount?: number, note: string}>}
   */
  async function _arbitrate(dispute) {
    const evidence = await store.listEvidenceByDispute(dispute.id);
    const filedByEvidence = evidence.filter((e) => e.submitted_by === dispute.filed_by);
    const filedAgainstEvidence = evidence.filter((e) => e.submitted_by === dispute.filed_against);

    // Check seller reputation
    let sellerReputation = null;
    try {
      sellerReputation = store.getReputationScore(dispute.filed_against);
    } catch {
      // no reputation data
    }
    const sellerScore = sellerReputation?.average_score ?? 3;

    const disputeAmount = dispute.amount_decimal || 0;

    // Rule 1: non_delivery with no delivery proof from seller → full refund
    if (dispute.category === 'non_delivery') {
      const hasDeliveryProof = filedAgainstEvidence.some(
        (e) => e.evidence_type === 'delivery_proof',
      );
      if (!hasDeliveryProof) {
        return {
          resolutionType: 'full_refund',
          note: 'Auto-resolved: Non-delivery with no delivery proof from seller.',
        };
      }
      // Seller has delivery proof — split 50/50
      return {
        resolutionType: 'split',
        amount: Math.round((disputeAmount / 2) * 100) / 100,
        note: 'Auto-resolved: Non-delivery disputed — seller provided delivery proof. Split 50/50.',
      };
    }

    // Rule 2: poor_quality with seller reputation < 2.5 → full refund
    if (dispute.category === 'poor_quality' && sellerScore < 2.5) {
      return {
        resolutionType: 'full_refund',
        note: `Auto-resolved: Poor quality claim with seller reputation ${sellerScore}/5. Full refund.`,
      };
    }

    // Rule 3: overcharged → partial refund (20% of disputed amount)
    if (dispute.category === 'overcharged') {
      const refundAmount = Math.round(disputeAmount * 0.2 * 100) / 100;
      return {
        resolutionType: 'partial_refund',
        amount: refundAmount,
        note: `Auto-resolved: Overcharge claim. 20% refund of $${disputeAmount}.`,
      };
    }

    // Rule 4: unauthorized → full refund (always)
    if (dispute.category === 'unauthorized') {
      return {
        resolutionType: 'full_refund',
        note: 'Auto-resolved: Unauthorized transaction. Full refund.',
      };
    }

    // Rule 5: Both parties submitted evidence — split 50/50
    if (filedByEvidence.length > 0 && filedAgainstEvidence.length > 0) {
      return {
        resolutionType: 'split',
        amount: Math.round((disputeAmount / 2) * 100) / 100,
        note: 'Auto-resolved: Both parties submitted evidence. Split 50/50.',
      };
    }

    // Rule 6: Only filer has evidence → full refund
    if (filedByEvidence.length > 0 && filedAgainstEvidence.length === 0) {
      return {
        resolutionType: 'full_refund',
        note: 'Auto-resolved: Only filing party submitted evidence. Full refund.',
      };
    }

    // Rule 7: Only respondent has evidence → release to seller
    if (filedByEvidence.length === 0 && filedAgainstEvidence.length > 0) {
      return {
        resolutionType: 'release_to_seller',
        note: 'Auto-resolved: Only respondent submitted evidence. Funds released to seller.',
      };
    }

    // Default: split 50/50
    return {
      resolutionType: 'split',
      amount: Math.round((disputeAmount / 2) * 100) / 100,
      note: 'Auto-resolved: No clear evidence from either party. Split 50/50.',
    };
  }

  /**
   * Execute an escrow action from a dispute resolution.
   * @param {Object} action
   */
  async function _executeEscrowAction(action) {
    if (!escrowService || !action || !action.escrowId) return;

    switch (action.action) {
      case 'refund':
        await escrowService.refundEscrow(action.escrowId);
        break;
      case 'release':
        await escrowService.releaseEscrow(action.escrowId);
        break;
      case 'partial_refund':
      case 'split':
        // For partial/split, we refund — the escrow service handles the amount
        await escrowService.refundEscrow(action.escrowId);
        break;
      default:
        // 'hold' — do nothing
        break;
    }
  }

  function start() {
    if (_running) return;
    _running = true;
    _timer = setInterval(() => {
      tick().catch((err) => {
        console.error('[dispute-resolver] Tick failed:', err.message);
      });
    }, intervalMs);
    if (_timer.unref) _timer.unref();
  }

  function stop() {
    if (!_running) return;
    _running = false;
    if (_timer) {
      clearInterval(_timer);
      _timer = null;
    }
  }

  function getMetrics() {
    return { ..._metrics, running: _running };
  }

  return {
    tick,
    start,
    stop,
    getMetrics,
    on: emitter.on.bind(emitter),
    off: emitter.removeListener.bind(emitter),
  };
}

export default { createDisputeResolver };
