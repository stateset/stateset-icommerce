/**
 * Fraud Commands Module
 */

function parseJsonArg(value, label) {
  try {
    return JSON.parse(value);
  } catch (error) {
    throw new Error(`Invalid ${label} JSON: ${error.message}`);
  }
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'assess': {
      // The engine does not detect signals itself: the caller supplies them
      // (`[{ signalType, score, details }]`) and it records them, sets the risk
      // score to the highest signal score and derives the decision.
      const [orderId, signalsJson] = args;
      if (!orderId) throw new Error('Usage: fraud assess <orderId> [signalsJson]');
      const signals = signalsJson ? parseJsonArg(signalsJson, 'signals') : [];
      if (!Array.isArray(signals)) throw new Error('signals must be a JSON array');
      const assessment = await commerce.fraud.createAssessment({
        orderId,
        signals: signals.map(({ signalType, score, details }) => ({
          signalType,
          score: Number(score),
          details,
        })),
      });
      return formatAssessment(assessment, { jsonOutput });
    }

    case 'assessment': {
      // Assessments are keyed by order: there is no separate assessment ID.
      const orderId = args[0];
      if (!orderId) throw new Error('Usage: fraud assessment <orderId>');
      const assessment = await commerce.fraud.getAssessment(orderId);
      if (!assessment) throw new Error(`Fraud assessment not found for order: ${orderId}`);
      return formatAssessment(assessment, { jsonOutput });
    }

    case 'signals': {
      const [orderId, minRiskScoreRaw, limitRaw] = args;
      const limit = limitRaw ? Number.parseInt(limitRaw, 10) : undefined;
      const minRiskScore = minRiskScoreRaw ? Number(minRiskScoreRaw) : undefined;
      if (minRiskScore !== undefined && !Number.isFinite(minRiskScore)) {
        throw new Error(`Invalid minRiskScore: ${minRiskScoreRaw}`);
      }
      let assessments;
      if (orderId) {
        const assessment = await commerce.fraud.getAssessment(orderId);
        assessments = assessment ? [assessment] : [];
        if (minRiskScore !== undefined) {
          assessments = assessments.filter((a) => a.riskScore >= minRiskScore);
        }
      } else {
        assessments = await commerce.fraud.listAssessments({ minRiskScore });
      }
      const signals = assessments.flatMap((a) => a.signals);
      const limited = Number.isInteger(limit) && limit > 0 ? signals.slice(0, limit) : signals;
      return formatSignals(limited, { output, jsonOutput });
    }

    case 'create-rule': {
      const payloadJson = args[0];
      if (!payloadJson) throw new Error('Usage: fraud create-rule <payloadJson>');
      const rule = await commerce.fraud.createRule(parseJsonArg(payloadJson, 'payload'));
      return {
        rule,
        formatted: `Created fraud rule ${rule.id || rule.name}`,
      };
    }

    case 'update-rule': {
      const [ruleId, updatesJson] = args;
      if (!ruleId || !updatesJson) {
        throw new Error('Usage: fraud update-rule <ruleId> <updatesJson>');
      }
      const rule = await commerce.fraud.updateRule(ruleId, parseJsonArg(updatesJson, 'updates'));
      return {
        rule,
        formatted: `Updated fraud rule ${rule.id || ruleId}`,
      };
    }

    case 'review': {
      const [orderId, decision, reviewer, ...noteParts] = args;
      if (!orderId || !decision || !reviewer) {
        throw new Error('Usage: fraud review <orderId> <accept|review|reject> <reviewer> [notes]');
      }
      const assessment = await commerce.fraud.reviewAssessment(
        orderId,
        decision,
        reviewer,
        noteParts.join(' ') || undefined,
      );
      return {
        assessment,
        formatted: `Recorded ${decision} decision for the fraud assessment of order ${orderId}`,
      };
    }

    default:
      throw new Error(
        `Unknown action: fraud ${action}\n\n` +
          'Available actions:\n' +
          '  assess <orderId> [signalsJson]        Record fraud assessment from signals\n' +
          '  assessment <orderId>                  Get fraud assessment\n' +
          '  signals [orderId] [minRiskScore] [limit]  List fraud signals\n' +
          '  create-rule <payloadJson>             Create fraud rule\n' +
          '  update-rule <ruleId> <updatesJson>    Update fraud rule\n' +
          '  review <orderId> <decision> <reviewer> [notes]  Review flagged order',
      );
  }
}

function formatAssessment(assessment, { jsonOutput }) {
  if (jsonOutput) return assessment;
  return {
    assessment,
    formatted:
      `Fraud assessment for order ${assessment.orderId}\n` +
      `${'-'.repeat(38)}\n` +
      `Risk score:     ${assessment.riskScore}\n` +
      `Decision:       ${assessment.decision}\n` +
      `Needs review:   ${assessment.needsReview ? 'yes' : 'no'}\n` +
      `Reviewed by:    ${assessment.reviewedBy || 'N/A'}\n` +
      `Signals:        ${Array.isArray(assessment.signals) ? assessment.signals.length : 0}`,
  };
}

function formatSignals(signals, { output, jsonOutput }) {
  if (jsonOutput) return signals;
  if (signals.length === 0) return { formatted: 'No fraud signals found.' };
  const formatted = output.table(signals, [
    { key: 'orderId', header: 'Order' },
    { key: 'signalType', header: 'Type' },
    { key: 'score', header: 'Score', align: 'right' },
    { key: 'details', header: 'Details' },
    { key: 'detectedAt', header: 'Detected' },
  ]);
  return { signals, formatted };
}

export const metadata = {
  name: 'fraud',
  aliases: ['risk', 'fraud-review'],
  description: 'Fraud assessment and rule-management commands',
  actions: {
    assess: {
      description: 'Record an order fraud assessment from caller-supplied signals',
      args: ['<orderId>', '[signalsJson]'],
    },
    assessment: { description: 'Get fraud assessment', args: ['<orderId>'] },
    signals: {
      description: 'List fraud signals',
      args: ['[orderId]', '[minRiskScore]', '[limit]'],
    },
    'create-rule': { description: 'Create fraud rule', args: ['<payloadJson>'] },
    'update-rule': { description: 'Update fraud rule', args: ['<ruleId>', '<updatesJson>'] },
    review: {
      description: 'Review flagged order',
      args: ['<orderId>', '<decision>', '<reviewer>', '[notes]'],
    },
  },
};

export default { execute, metadata };
