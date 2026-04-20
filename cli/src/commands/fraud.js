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
      const [orderId, customerIp, deviceFingerprint, billingAddressJson, shippingAddressJson] =
        args;
      if (!orderId) {
        throw new Error(
          'Usage: fraud assess <orderId> [customerIp] [deviceFingerprint] [billingAddressJson] [shippingAddressJson]',
        );
      }
      const assessment = await commerce.fraud.assessOrder({
        orderId,
        customerIp: customerIp || undefined,
        deviceFingerprint: deviceFingerprint || undefined,
        billingAddress: billingAddressJson
          ? parseJsonArg(billingAddressJson, 'billingAddress')
          : undefined,
        shippingAddress: shippingAddressJson
          ? parseJsonArg(shippingAddressJson, 'shippingAddress')
          : undefined,
      });
      return formatAssessment(assessment, { jsonOutput });
    }

    case 'assessment': {
      const assessmentId = args[0];
      if (!assessmentId) throw new Error('Usage: fraud assessment <assessmentId>');
      const assessment = await commerce.fraud.getAssessment(assessmentId);
      if (!assessment) throw new Error(`Fraud assessment not found: ${assessmentId}`);
      return formatAssessment(assessment, { jsonOutput });
    }

    case 'signals': {
      const [orderId, riskLevel, limitRaw] = args;
      const signals = await commerce.fraud.listSignals({
        orderId: orderId || undefined,
        riskLevel: riskLevel || undefined,
      });
      const limit = limitRaw ? Number.parseInt(limitRaw, 10) : undefined;
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
      const [assessmentId, decision, reason, ...noteParts] = args;
      if (!assessmentId || !decision || !reason) {
        throw new Error('Usage: fraud review <assessmentId> <decision> <reason> [reviewerNote]');
      }
      const assessment = await commerce.fraud.reviewOrder({
        assessmentId,
        decision,
        reason,
        reviewerNote: noteParts.join(' ') || undefined,
      });
      return {
        assessment,
        formatted: `Recorded ${decision} decision for fraud assessment ${assessmentId}`,
      };
    }

    default:
      throw new Error(
        `Unknown action: fraud ${action}\n\n` +
          'Available actions:\n' +
          '  assess <orderId> [customerIp] [deviceFingerprint] [billingAddressJson] [shippingAddressJson]\n' +
          '  assessment <assessmentId>             Get fraud assessment\n' +
          '  signals [orderId] [riskLevel] [limit] List fraud signals\n' +
          '  create-rule <payloadJson>             Create fraud rule\n' +
          '  update-rule <ruleId> <updatesJson>    Update fraud rule\n' +
          '  review <assessmentId> <decision> <reason> [reviewerNote]  Review flagged order',
      );
  }
}

function formatAssessment(assessment, { jsonOutput }) {
  if (jsonOutput) return assessment;
  return {
    assessment,
    formatted:
      `Fraud assessment: ${assessment.id}\n` +
      `${'-'.repeat(38)}\n` +
      `Order:          ${assessment.orderId}\n` +
      `Risk score:     ${assessment.riskScore}\n` +
      `Risk level:     ${assessment.riskLevel}\n` +
      `Recommendation: ${assessment.recommendation}\n` +
      `Signals:        ${Array.isArray(assessment.signals) ? assessment.signals.length : 0}`,
  };
}

function formatSignals(signals, { output, jsonOutput }) {
  if (jsonOutput) return signals;
  if (signals.length === 0) return { formatted: 'No fraud signals found.' };
  const formatted = output.table(signals, [
    { key: 'id', header: 'ID' },
    { key: 'orderId', header: 'Order' },
    { key: 'type', header: 'Type' },
    { key: 'severity', header: 'Severity' },
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
      description: 'Assess order fraud risk',
      args: [
        '<orderId>',
        '[customerIp]',
        '[deviceFingerprint]',
        '[billingAddressJson]',
        '[shippingAddressJson]',
      ],
    },
    assessment: { description: 'Get fraud assessment', args: ['<assessmentId>'] },
    signals: { description: 'List fraud signals', args: ['[orderId]', '[riskLevel]', '[limit]'] },
    'create-rule': { description: 'Create fraud rule', args: ['<payloadJson>'] },
    'update-rule': { description: 'Update fraud rule', args: ['<ruleId>', '<updatesJson>'] },
    review: {
      description: 'Review flagged order',
      args: ['<assessmentId>', '<decision>', '<reason>', '[reviewerNote]'],
    },
  },
};

export default { execute, metadata };
