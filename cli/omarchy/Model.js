var MAX_OUTPUT_CHARS = 65536
var MAX_ERROR_CHARS = 4096
var MAX_TEXT_CHARS = 160
var MAX_PATH_CHARS = 1024
var MAX_COUNT = 999999999
var MAX_JSON_DEPTH = 8
var MAX_JSON_FIELDS = 256
var MAX_JSON_ARRAYS = 16
var MAX_JSON_ARRAY_DEPTH = 3
var MAX_JSON_ARRAY_SEPARATORS = 127
// BEGIN GENERATED CONTRACT — run node scripts/generate-contract.mjs
var STATUS_SCHEMA_VERSION = 1
var CONTROLLER_SERIES = "1.30"
var STATUS_CAPABILITIES = ["status","dashboard","agent","attention","remediate","backup","doctor","agent-config","mcp-service"]
var SNAPSHOT_STATE_VERSION = 1
var SNAPSHOT_MAX_AGE_MS = 86400000
var SNAPSHOT_MAX_BYTES = 8192
// END GENERATED CONTRACT

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

function boundedBytes(value) {
  if (typeof value === "string" && !/^\d+$/.test(value)) return 0
  if (typeof value !== "string" && typeof value !== "number") return 0
  var parsed = Number(value)
  if (!isFinite(parsed) || parsed < 0) return 0
  return Math.min(9007199254740991, Math.floor(parsed))
}

function safeText(value, limit, fallback) {
  if (typeof value !== "string") return fallback || ""
  var maximum = typeof limit === "number" && isFinite(limit) ? Math.max(0, limit) : MAX_TEXT_CHARS
  var bounded = value.slice(0, maximum)
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

function normalizeUnavailableSignals(value) {
  if (!Array.isArray(value)) return []
  var allowed = ["orders", "payments", "pendingReturns", "lowStockItems"]
  var result = []
  for (var index = 0; index < value.length && result.length < allowed.length; index += 1) {
    var signal = value[index]
    if (allowed.indexOf(signal) >= 0 && result.indexOf(signal) < 0) result.push(signal)
  }
  return result
}

function normalizeCapabilities(value) {
  if (!Array.isArray(value)) return []
  var result = []
  for (var index = 0; index < value.length && result.length < STATUS_CAPABILITIES.length; index += 1) {
    if (typeof value[index] !== "string") continue
    var capability = value[index]
    if (STATUS_CAPABILITIES.indexOf(capability) >= 0 && result.indexOf(capability) < 0) result.push(capability)
  }
  return result
}

function controllerVersionCompatible(value) {
  if (value === undefined || value === null || value === "") return true
  if (typeof value !== "string") return false
  var match = value.match(/^(\d+)\.(\d+)\.(\d+)(?:[-+][0-9A-Za-z.-]+)?$/)
  return match !== null && match[1] + "." + match[2] === CONTROLLER_SERIES
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
  if (source.schemaVersion !== undefined && source.schemaVersion !== STATUS_SCHEMA_VERSION) {
    throw new Error("Unsupported status schema version")
  }
  if (!controllerVersionCompatible(source.controllerVersion)) {
    throw new Error("Unsupported controller version")
  }
  var ready = source.ok === true
  var unavailableSignals = normalizeUnavailableSignals(source.unavailableSignals)
  var capabilitiesKnown = Array.isArray(source.capabilities)
  return {
    ok: ready,
    schemaVersion: source.schemaVersion === STATUS_SCHEMA_VERSION ? STATUS_SCHEMA_VERSION : 0,
    configured: source.configured === true,
    dbPath: safeText(source.dbPath, MAX_PATH_CHARS, ""),
    mode: source.mode === "governed-apply" ? "governed-apply" : "preview",
    message: safeText(source.message, MAX_TEXT_CHARS, ready ? "Store ready" : "Store unavailable"),
    controllerVersion: safeText(source.controllerVersion, 64, ""),
    capabilitiesKnown: capabilitiesKnown,
    capabilities: normalizeCapabilities(source.capabilities),
    sizeBytes: boundedBytes(source.sizeBytes),
    counts: normalizeCounts(source.counts),
    alerts: normalizeAlerts(source.alerts),
    signalsComplete: source.signalsComplete !== false && unavailableSignals.length === 0,
    unavailableSignals: unavailableSignals
  }
}

function createSnapshotState(value, savedAt) {
  var snapshot = normalizeStatus(value)
  var timestamp = Number(savedAt)
  if (!snapshot.ok || !snapshot.configured || !isFinite(timestamp) || timestamp <= 0) return null
  return { version: SNAPSHOT_STATE_VERSION, savedAt: timestamp, snapshot: snapshot }
}

function parseSnapshotState(text, now) {
  var fallback = { ok: false, savedAt: 0, snapshot: null }
  if (typeof text !== "string" || text.trim() === "" || text.length > SNAPSHOT_MAX_BYTES) return fallback
  try {
    var source = record(JSON.parse(text))
    if (source.version !== SNAPSHOT_STATE_VERSION) return fallback
    var savedAt = Number(source.savedAt)
    var current = Number(now)
    if (!isFinite(current)) current = Date.now()
    if (!isFinite(savedAt) || savedAt <= 0 || savedAt > current + 300000
        || current - savedAt > SNAPSHOT_MAX_AGE_MS) return fallback
    var snapshot = normalizeStatus(source.snapshot)
    if (!snapshot.ok || !snapshot.configured) return fallback
    return { ok: true, savedAt: savedAt, snapshot: snapshot }
  } catch (error) {
    return fallback
  }
}

function normalizeServiceStatus(value) {
  var source = record(value)
  var states = ["active", "activating", "deactivating", "failed", "inactive", "not-installed", "removed", "unknown"]
  var state = typeof source.state === "string" && states.indexOf(source.state) >= 0 ? source.state : "unknown"
  var installed = source.installed === true
  if (!installed) {
    return {
      installed: false,
      active: false,
      state: state === "removed" ? "removed" : "not-installed"
    }
  }
  if (state === "not-installed" || state === "removed") state = "unknown"
  return {
    installed: true,
    active: source.active === true && state === "active",
    state: state
  }
}

function serviceActionCommand(action) {
  var allowed = ["install", "start", "stop", "restart"]
  if (allowed.indexOf(action) < 0) return []
  // Keep a stable executable boundary if the controller disappears after its
  // last successful probe. `timeout` exits 127 instead of stranding the UI.
  var command = ["/usr/bin/timeout", "--signal=TERM", "--kill-after=1s", "10s",
    "stateset-omarchy", "service", action]
  if (action !== "install") command.push("--json")
  return command
}

function serviceActionLabel(action, success) {
  var labels = {
    install: success ? "MCP service installed" : "Unable to install MCP service",
    start: success ? "MCP service started" : "Unable to start MCP service",
    stop: success ? "MCP service stopped" : "Unable to stop MCP service",
    restart: success ? "MCP service restarted" : "Unable to restart MCP service"
  }
  return labels[action] || (success ? "MCP action completed" : "MCP action failed")
}

function parseServiceStatusJson(text) {
  validateJsonEnvelope(text)
  var value = JSON.parse(text)
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("Service response must be an object")
  }
  return normalizeServiceStatus(value)
}

function classifyFailure(exitCode, output, timedOut, truncated) {
  if (timedOut === true || exitCode === 124) return "timeout"
  if (truncated === true) return "oversized-response"
  var text = String(output || "").toLowerCase()
  if (exitCode === 127 || /command not found|no such file or directory/.test(text)) return "controller-missing"
  if (exitCode !== 0) return "controller-error"
  return "invalid-response"
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
  var maximum = typeof limit === "number" && isFinite(limit) ? Math.max(1, limit) : MAX_OUTPUT_CHARS
  var existing = typeof current === "string" ? current : ""
  var incoming = typeof chunk === "string" ? chunk : String(chunk === null || chunk === undefined ? "" : chunk)
  var remaining = maximum - existing.length
  if (remaining <= 0) return { text: existing, truncated: incoming.length > 0 }
  return {
    text: existing + incoming.slice(0, remaining),
    truncated: incoming.length > remaining
  }
}

function formatCount(value) {
  var count = boundedCount(value)
  if (count < 1000) return String(count)
  if (count < 10000) return (Math.floor(count / 100) / 10).toFixed(1).replace(/\.0$/, "") + "K"
  if (count < 1000000) return Math.floor(count / 1000) + "K"
  if (count < 10000000) return (Math.floor(count / 100000) / 10).toFixed(1).replace(/\.0$/, "") + "M"
  return Math.floor(count / 1000000) + "M"
}

function formatExactCount(value) {
  return String(boundedCount(value)).replace(/\B(?=(\d{3})+(?!\d))/g, ",")
}

function formatBytes(value) {
  var bytes = boundedBytes(value)
  if (bytes < 1024) return bytes + " B"
  var units = ["KiB", "MiB", "GiB", "TiB"]
  var amount = bytes
  var unit = 0
  while (amount >= 1024 && unit < units.length) {
    amount /= 1024
    unit += 1
  }
  var precision = amount < 10 ? 1 : 0
  return amount.toFixed(precision).replace(/\.0$/, "") + " " + units[unit - 1]
}

function freshnessLabel(value, now) {
  var timestamp = value instanceof Date ? value.getTime() : Number(value)
  var current = now instanceof Date ? now.getTime() : Number(now)
  if (!isFinite(timestamp) || timestamp <= 0) return "Not updated yet"
  if (!isFinite(current)) current = Date.now()
  var seconds = Math.max(0, Math.floor((current - timestamp) / 1000))
  if (seconds < 10) return "Updated just now"
  if (seconds < 60) return "Updated " + seconds + "s ago"
  var minutes = Math.floor(seconds / 60)
  if (minutes < 60) return "Updated " + minutes + "m ago"
  var hours = Math.floor(minutes / 60)
  return "Updated " + hours + "h ago"
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

function attentionHeadline(value) {
  var total = normalizeAlerts(value).total
  return total + (total === 1 ? " NEEDS ATTENTION" : " NEED ATTENTION")
}

function shouldNotify(previous, next, hasBaseline) {
  if (hasBaseline !== true) return false
  var before = normalizeAlerts(previous)
  var after = normalizeAlerts(next)
  return after.failedPayments > before.failedPayments
    || after.lowStock > before.lowStock
    || after.pendingReturns > before.pendingReturns
}

function notificationCandidate(previous, next, policy) {
  var before = normalizeAlerts(previous)
  var after = normalizeAlerts(next)
  var options = record(policy)
  return {
    pendingOrders: before.pendingOrders,
    failedPayments: options.failedPayments === false ? before.failedPayments : after.failedPayments,
    pendingReturns: options.pendingReturns === false ? before.pendingReturns : after.pendingReturns,
    lowStock: options.lowStock === false ? before.lowStock : after.lowStock
  }
}

function normalizeNotificationDelta(value) {
  var source = record(value)
  return {
    failedPayments: boundedCount(source.failedPayments),
    pendingReturns: boundedCount(source.pendingReturns),
    lowStock: boundedCount(source.lowStock)
  }
}

function filterNotificationDelta(value, policy) {
  var pending = normalizeNotificationDelta(value)
  var enabled = record(policy)
  return {
    failedPayments: enabled.failedPayments === false ? 0 : pending.failedPayments,
    pendingReturns: enabled.pendingReturns === false ? 0 : pending.pendingReturns,
    lowStock: enabled.lowStock === false ? 0 : pending.lowStock
  }
}

function notificationDelta(previous, next, policy) {
  var before = normalizeAlerts(previous)
  var candidate = notificationCandidate(previous, next, policy)
  return {
    failedPayments: Math.max(0, candidate.failedPayments - before.failedPayments),
    pendingReturns: Math.max(0, candidate.pendingReturns - before.pendingReturns),
    lowStock: Math.max(0, candidate.lowStock - before.lowStock)
  }
}

function mergePendingNotifications(pending, delta, current) {
  var queued = normalizeNotificationDelta(pending)
  var incoming = normalizeNotificationDelta(delta)
  var active = normalizeAlerts(current)
  return {
    failedPayments: active.failedPayments > 0
      ? Math.min(active.failedPayments, queued.failedPayments + incoming.failedPayments) : 0,
    pendingReturns: active.pendingReturns > 0
      ? Math.min(active.pendingReturns, queued.pendingReturns + incoming.pendingReturns) : 0,
    lowStock: active.lowStock > 0
      ? Math.min(active.lowStock, queued.lowStock + incoming.lowStock) : 0
  }
}

function hasPendingNotifications(value) {
  var pending = normalizeNotificationDelta(value)
  return pending.failedPayments > 0 || pending.pendingReturns > 0 || pending.lowStock > 0
}

function pendingNotificationSummary(value) {
  var pending = normalizeNotificationDelta(value)
  var parts = []
  if (pending.failedPayments > 0) parts.push("+" + pending.failedPayments + " failed payment" + (pending.failedPayments === 1 ? "" : "s"))
  if (pending.lowStock > 0) parts.push("+" + pending.lowStock + " low-stock SKU" + (pending.lowStock === 1 ? "" : "s"))
  if (pending.pendingReturns > 0) parts.push("+" + pending.pendingReturns + " pending return" + (pending.pendingReturns === 1 ? "" : "s"))
  return parts.join(" · ")
}

function parseNotificationState(text) {
  var fallback = { lastNotificationAt: 0, pending: normalizeNotificationDelta({}) }
  if (typeof text !== "string" || text.trim() === "") return fallback
  if (text.length > MAX_ERROR_CHARS) return fallback
  try {
    var source = record(JSON.parse(text))
    if (source.version !== undefined && source.version !== 1) return fallback
    var last = Number(source.lastNotificationAt)
    return {
      lastNotificationAt: isFinite(last) && last > 0 ? last : 0,
      pending: normalizeNotificationDelta(source.pending)
    }
  } catch (error) {
    return fallback
  }
}

function cooldownElapsed(lastNotificationAt, now, cooldownMinutes) {
  var last = Number(lastNotificationAt)
  var current = Number(now)
  var minutes = Number(cooldownMinutes)
  if (!isFinite(last) || last <= 0) return true
  if (!isFinite(current)) current = Date.now()
  if (!isFinite(minutes)) minutes = 15
  minutes = Math.max(1, Math.min(240, minutes))
  return current - last >= minutes * 60000
}

function cooldownRemainingMs(lastNotificationAt, now, cooldownMinutes) {
  if (cooldownElapsed(lastNotificationAt, now, cooldownMinutes)) return 0
  var last = Number(lastNotificationAt)
  var current = Number(now)
  var minutes = Math.max(1, Math.min(240, Number(cooldownMinutes) || 15))
  if (!isFinite(last) || !isFinite(current) || last <= 0) return 0
  // Bound corrupt or clock-skewed future timestamps to one cooldown so they
  // cannot overflow a QML Timer or suppress notifications indefinitely.
  var elapsed = Math.max(0, current - last)
  return Math.max(0, Math.ceil(minutes * 60000 - elapsed))
}

function retryIntervalSeconds(baseInterval, consecutiveFailures) {
  var base = Number(baseInterval)
  var failures = Number(consecutiveFailures)
  if (!isFinite(base)) base = 120
  if (!isFinite(failures) || failures < 1) failures = 0
  base = Math.max(30, Math.min(1800, Math.floor(base)))
  var exponent = Math.max(0, Math.min(4, Math.floor(failures) - 1))
  return Math.min(1800, base * Math.pow(2, exponent))
}

function retryCountdownLabel(value, now) {
  var timestamp = value instanceof Date ? value.getTime() : Number(value)
  var current = now instanceof Date ? now.getTime() : Number(now)
  if (!isFinite(timestamp) || timestamp <= 0) return ""
  if (!isFinite(current)) current = Date.now()
  var seconds = Math.max(0, Math.ceil((timestamp - current) / 1000))
  if (seconds <= 1) return "Retrying now"
  if (seconds < 60) return "Retrying in " + seconds + "s"
  return "Retrying in " + Math.ceil(seconds / 60) + "m"
}

function notificationSummary(previous, next) {
  var before = normalizeAlerts(previous)
  var after = normalizeAlerts(next)
  var parts = []
  var failedPayments = Math.max(0, after.failedPayments - before.failedPayments)
  var lowStock = Math.max(0, after.lowStock - before.lowStock)
  var pendingReturns = Math.max(0, after.pendingReturns - before.pendingReturns)
  if (failedPayments > 0) parts.push("+" + failedPayments + " failed payment" + (failedPayments === 1 ? "" : "s"))
  if (lowStock > 0) parts.push("+" + lowStock + " low-stock SKU" + (lowStock === 1 ? "" : "s"))
  if (pendingReturns > 0) parts.push("+" + pendingReturns + " pending return" + (pendingReturns === 1 ? "" : "s"))
  return parts.join(" · ")
}

if (typeof module !== "undefined") {
  module.exports = {
    MAX_OUTPUT_CHARS: MAX_OUTPUT_CHARS,
    MAX_ERROR_CHARS: MAX_ERROR_CHARS,
    STATUS_SCHEMA_VERSION: STATUS_SCHEMA_VERSION,
    CONTROLLER_SERIES: CONTROLLER_SERIES,
    STATUS_CAPABILITIES: STATUS_CAPABILITIES,
    SNAPSHOT_MAX_AGE_MS: SNAPSHOT_MAX_AGE_MS,
    appendBounded: appendBounded,
    formatBytes: formatBytes,
    formatCount: formatCount,
    formatExactCount: formatExactCount,
    freshnessLabel: freshnessLabel,
    filterNotificationDelta: filterNotificationDelta,
    normalizeAlerts: normalizeAlerts,
    normalizeCounts: normalizeCounts,
    normalizeCapabilities: normalizeCapabilities,
    normalizeNotificationDelta: normalizeNotificationDelta,
    normalizeStatus: normalizeStatus,
    normalizeServiceStatus: normalizeServiceStatus,
    normalizeUnavailableSignals: normalizeUnavailableSignals,
    parseStatusJson: parseStatusJson,
    parseSnapshotState: parseSnapshotState,
    parseServiceStatusJson: parseServiceStatusJson,
    serviceActionCommand: serviceActionCommand,
    serviceActionLabel: serviceActionLabel,
    classifyFailure: classifyFailure,
    controllerVersionCompatible: controllerVersionCompatible,
    createSnapshotState: createSnapshotState,
    safeText: safeText,
    attentionSummary: attentionSummary,
    attentionHeadline: attentionHeadline,
    cooldownElapsed: cooldownElapsed,
    cooldownRemainingMs: cooldownRemainingMs,
    hasPendingNotifications: hasPendingNotifications,
    mergePendingNotifications: mergePendingNotifications,
    notificationCandidate: notificationCandidate,
    notificationDelta: notificationDelta,
    notificationSummary: notificationSummary,
    parseNotificationState: parseNotificationState,
    pendingNotificationSummary: pendingNotificationSummary,
    retryIntervalSeconds: retryIntervalSeconds,
    retryCountdownLabel: retryCountdownLabel,
    shouldNotify: shouldNotify
  }
}
