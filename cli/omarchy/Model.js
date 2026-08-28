var MAX_OUTPUT_CHARS = 65536
var MAX_TEXT_CHARS = 160
var MAX_PATH_CHARS = 1024
var MAX_COUNT = 999999999
var MAX_JSON_DEPTH = 8
var MAX_JSON_FIELDS = 256
var MAX_JSON_ARRAYS = 16
var MAX_JSON_ARRAY_DEPTH = 3
var MAX_JSON_ARRAY_SEPARATORS = 127

function record(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value) ? value : ({})
}

function boundedCount(value) {
  if (typeof value === "string" && !/^\d+$/.test(value)) return 0
  if (typeof value !== "string" && typeof value !== "number") return 0
  var parsed = Number(value)
  if (!isFinite(parsed) || parsed < 0) return 0
  return Math.min(MAX_COUNT, Math.floor(parsed))
}

function safeText(value, limit, fallback) {
  if (typeof value !== "string") return fallback || ""
  var bounded = value.slice(0, Math.max(0, limit || MAX_TEXT_CHARS))
  return bounded
    .replace(/[\u0000-\u001f\u007f-\u009f\u202a-\u202e\u2066-\u2069]/g, " ")
    .replace(/&/g, "＆")
    .replace(/</g, "‹")
    .replace(/>/g, "›")
    .replace(/\s+/g, " ")
    .trim() || fallback || ""
}

function normalizeCounts(value) {
  var source = record(value)
  return {
    orders: boundedCount(source.orders),
    customers: boundedCount(source.customers),
    products: boundedCount(source.products),
    returns: boundedCount(source.returns),
    payments: boundedCount(source.payments)
  }
}

function normalizeAlerts(value) {
  var source = record(value)
  var alerts = {
    pendingOrders: boundedCount(source.pendingOrders),
    failedPayments: boundedCount(source.failedPayments),
    pendingReturns: boundedCount(source.pendingReturns),
    lowStock: boundedCount(source.lowStock)
  }
  alerts.total = Math.min(MAX_COUNT,
    alerts.pendingOrders + alerts.failedPayments + alerts.pendingReturns + alerts.lowStock)
  return alerts
}

function validateJsonEnvelope(text) {
  if (typeof text !== "string" || text.length === 0 || text.length > MAX_OUTPUT_CHARS) {
    throw new Error("Status response size is invalid")
  }
  var depth = 0
  var fields = 0
  var arrayDepth = 0
  var arrays = 0
  var arraySeparators = 0
  var quoted = false
  var escaped = false
  for (var index = 0; index < text.length; index += 1) {
    var character = text.charAt(index)
    if (quoted) {
      if (escaped) escaped = false
      else if (character === "\\") escaped = true
      else if (character === "\"") quoted = false
      continue
    }
    if (character === "\"") quoted = true
    else if (character === "{" || character === "[") {
      depth += 1
      if (depth > MAX_JSON_DEPTH) throw new Error("Status response is too deeply nested")
      if (character === "[") {
        arrays += 1
        arrayDepth += 1
        if (arrays > MAX_JSON_ARRAYS || arrayDepth > MAX_JSON_ARRAY_DEPTH) {
          throw new Error("Status response has too many arrays")
        }
      }
    } else if (character === "}" || character === "]") {
      depth -= 1
      if (depth < 0) throw new Error("Status response is malformed")
      if (character === "]") {
        arrayDepth -= 1
        if (arrayDepth < 0) throw new Error("Status response is malformed")
      }
    } else if (character === ":") {
      fields += 1
      if (fields > MAX_JSON_FIELDS) throw new Error("Status response has too many fields")
    } else if (character === "," && arrayDepth > 0) {
      arraySeparators += 1
      if (arraySeparators > MAX_JSON_ARRAY_SEPARATORS) {
        throw new Error("Status response arrays contain too many values")
      }
    }
  }
  if (quoted || depth !== 0 || arrayDepth !== 0) throw new Error("Status response is malformed")
}

function normalizeStatus(value) {
  var source = record(value)
  var ready = source.ok === true
  return {
    ok: ready,
    configured: source.configured === true,
    dbPath: safeText(source.dbPath, MAX_PATH_CHARS, ""),
    mode: source.mode === "governed-apply" ? "governed-apply" : "preview",
    message: safeText(source.message, MAX_TEXT_CHARS, ready ? "Store ready" : "Store unavailable"),
    counts: normalizeCounts(source.counts),
    alerts: normalizeAlerts(source.alerts)
  }
}

function parseStatusJson(text) {
  validateJsonEnvelope(text)
  var value = JSON.parse(text)
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("Status response must be an object")
  }
  return normalizeStatus(value)
}

function appendBounded(current, chunk, limit) {
  var maximum = Math.max(1, limit || MAX_OUTPUT_CHARS)
  var existing = typeof current === "string" ? current : ""
  var incoming = typeof chunk === "string" ? chunk : String(chunk || "")
  var remaining = maximum - existing.length
  if (remaining <= 0) return { text: existing, truncated: incoming.length > 0 }
  return {
    text: existing + incoming.slice(0, remaining),
    truncated: incoming.length > remaining
  }
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
    MAX_OUTPUT_CHARS: MAX_OUTPUT_CHARS,
    appendBounded: appendBounded,
    normalizeAlerts: normalizeAlerts,
    normalizeCounts: normalizeCounts,
    normalizeStatus: normalizeStatus,
    parseStatusJson: parseStatusJson,
    safeText: safeText,
    attentionSummary: attentionSummary,
    shouldNotify: shouldNotify
  }
}
