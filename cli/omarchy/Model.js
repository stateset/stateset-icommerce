function number(value) {
  var parsed = parseInt(String(value || 0), 10)
  return isFinite(parsed) && parsed > 0 ? parsed : 0
}

function normalizeAlerts(value) {
  var source = value && typeof value === "object" ? value : ({})
  var alerts = {
    pendingOrders: number(source.pendingOrders),
    failedPayments: number(source.failedPayments),
    pendingReturns: number(source.pendingReturns),
    lowStock: number(source.lowStock)
  }
  alerts.total = alerts.pendingOrders + alerts.failedPayments + alerts.pendingReturns + alerts.lowStock
  return alerts
}

function attentionSummary(value) {
  var alerts = normalizeAlerts(value)
  var parts = []
  if (alerts.failedPayments > 0) parts.push(alerts.failedPayments + " failed payment" + (alerts.failedPayments === 1 ? "" : "s"))
  if (alerts.lowStock > 0) parts.push(alerts.lowStock + " low-stock SKU" + (alerts.lowStock === 1 ? "" : "s"))
  if (alerts.pendingReturns > 0) parts.push(alerts.pendingReturns + " pending return" + (alerts.pendingReturns === 1 ? "" : "s"))
  if (alerts.pendingOrders > 0) parts.push(alerts.pendingOrders + " pending order" + (alerts.pendingOrders === 1 ? "" : "s"))
  return parts.join(" · ")
}

function shouldNotify(previous, next, hasBaseline) {
  if (hasBaseline !== true) return false
  var before = normalizeAlerts(previous)
  var after = normalizeAlerts(next)
  return after.failedPayments > before.failedPayments
    || after.lowStock > before.lowStock
    || after.pendingReturns > before.pendingReturns
}

if (typeof module !== "undefined") {
  module.exports = {
    normalizeAlerts: normalizeAlerts,
    attentionSummary: attentionSummary,
    shouldNotify: shouldNotify
  }
}
