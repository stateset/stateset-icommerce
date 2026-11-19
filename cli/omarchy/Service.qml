import QtQuick
import Quickshell
import Quickshell.Io
import "Model.js" as Model

Item {
  id: root

  property var shell: null
  property var settings: ({})
  property bool refreshing: false
  property bool ready: false
  property bool configured: false
  property string dbPath: ""
  property string mode: "preview"
  property string message: "Discovering store"
  property string lastError: ""
  property string failureKind: ""
  property int statusSchemaVersion: 0
  property string controllerVersion: ""
  property bool capabilitiesKnown: false
  property var capabilities: []
  property double sizeBytes: 0
  property var counts: ({ orders: 0, customers: 0, products: 0, returns: 0, payments: 0 })
  property var alerts: ({ pendingOrders: 0, failedPayments: 0, pendingReturns: 0, lowStock: 0, total: 0 })
  property bool signalsComplete: true
  property var unavailableSignals: []
  property bool hasAlertBaseline: false
  property bool hasSnapshot: false
  property bool snapshotStateLoaded: false
  property bool snapshotRestored: false
  property date lastUpdated: new Date(0)
  property date lastAttempt: new Date(0)
  property date nextRefreshAt: new Date(0)
  property int consecutiveFailures: 0
  property bool mcpInstalled: false
  property bool mcpActive: false
  property bool mcpStatusKnown: false
  property string mcpState: "unknown"
  property bool mcpRefreshing: false
  property string mcpLastError: ""
  property date mcpLastUpdated: new Date(0)
  property bool actionRunning: false
  property string actionKind: ""
  property string actionStatus: ""
  property string actionError: ""
  property string _actionOutput: ""
  property bool _actionTruncated: false
  property bool _actionTimedOut: false
  property string _stdout: ""
  property string _stderr: ""
  property bool _stdoutTruncated: false
  property bool _timedOut: false
  property string _serviceOutput: ""
  property bool _serviceTruncated: false
  property double lastNotificationAt: 0
  property var pendingNotifications: ({ failedPayments: 0, pendingReturns: 0, lowStock: 0 })
  property bool notificationStateLoaded: false
  property bool notificationSnapshotReady: false

  readonly property string stateHome: Quickshell.env("XDG_STATE_HOME")
    || (Quickshell.env("HOME") + "/.local/state")
  readonly property string notificationStateDir: stateHome + "/stateset-icommerce"
  readonly property string notificationStatePath: notificationStateDir + "/notifications.json"
  readonly property string snapshotStatePath: notificationStateDir + "/snapshot.json"

  readonly property int refreshIntervalSec: {
    var value = parseInt(String(settings && settings.refreshIntervalSec || 120), 10)
    if (!isFinite(value)) value = 120
    return Math.max(30, Math.min(1800, value))
  }
  readonly property bool notificationsEnabled: settings && settings.notifications !== false
  readonly property int notificationCooldownMin: {
    var value = parseInt(String(settings && settings.notificationCooldownMin || 15), 10)
    if (!isFinite(value)) value = 15
    return Math.max(1, Math.min(240, value))
  }
  readonly property var notificationService: shell && typeof shell.firstPartyServiceFor === "function"
    ? shell.firstPartyServiceFor("omarchy.notifications") : null
  readonly property bool notificationsQuiet: notificationService
    ? notificationService.doNotDisturb === true : false

  function notificationPolicy() {
    return {
      failedPayments: settings && settings.notifyFailedPayments !== false,
      lowStock: settings && settings.notifyLowStock !== false,
      pendingReturns: settings && settings.notifyPendingReturns !== false
    }
  }

  function supportsCapability(capability) {
    return !capabilitiesKnown || capabilities.indexOf(capability) >= 0
  }

  function loadSnapshotState(text) {
    if (snapshotStateLoaded) return
    snapshotStateLoaded = true
    if (hasSnapshot) return
    var state = Model.parseSnapshotState(text, Date.now())
    if (!state.ok) return
    var value = state.snapshot
    ready = false
    configured = value.configured
    dbPath = value.dbPath
    mode = value.mode
    message = value.message
    statusSchemaVersion = value.schemaVersion
    controllerVersion = value.controllerVersion
    capabilitiesKnown = value.capabilitiesKnown
    capabilities = value.capabilities
    sizeBytes = value.sizeBytes
    counts = value.counts
    alerts = value.alerts
    signalsComplete = value.signalsComplete
    unavailableSignals = value.unavailableSignals
    hasAlertBaseline = true
    hasSnapshot = true
    snapshotRestored = true
    lastUpdated = new Date(state.savedAt)
    failureKind = "restored-snapshot"
  }

  function saveSnapshotState(value, savedAt) {
    var state = Model.createSnapshotState(value, savedAt)
    snapshotStateFile.setText(state === null ? "" : JSON.stringify(state, null, 2) + "\n")
    secureStateFiles()
  }

  function secureStateFiles() {
    statePermissionsTimer.restart()
  }

  function loadNotificationState(text) {
    if (notificationStateLoaded) return
    var value = Model.parseNotificationState(text)
    lastNotificationAt = value.lastNotificationAt
    pendingNotifications = value.pending
    notificationStateLoaded = true
  }

  function scheduleNotificationStateSave() {
    if (notificationStateLoaded) notificationStateSaveTimer.restart()
  }

  function flushNotificationState() {
    notificationStateFile.setText(JSON.stringify({
      version: 1,
      lastNotificationAt: lastNotificationAt,
      pending: Model.normalizeNotificationDelta(pendingNotifications)
    }, null, 2) + "\n")
    secureStateFiles()
  }

  function queueNotifications(previous, next, hasBaseline) {
    if (!notificationsEnabled) {
      pendingNotifications = Model.normalizeNotificationDelta({})
      scheduleNotificationStateSave()
      return
    }
    var policy = notificationPolicy()
    var delta = Model.normalizeNotificationDelta({})
    if (hasBaseline) {
      delta = Model.notificationDelta(previous, next, policy)
    }
    pendingNotifications = Model.mergePendingNotifications(
      Model.filterNotificationDelta(pendingNotifications, policy), delta, next)
    notificationSnapshotReady = true
    scheduleNotificationStateSave()
    deliverPendingNotifications()
  }

  function deliverPendingNotifications() {
    var filtered = Model.filterNotificationDelta(pendingNotifications, notificationPolicy())
    if (filtered.failedPayments !== pendingNotifications.failedPayments
        || filtered.pendingReturns !== pendingNotifications.pendingReturns
        || filtered.lowStock !== pendingNotifications.lowStock) {
      pendingNotifications = filtered
      scheduleNotificationStateSave()
    }
    if (!notificationStateLoaded || !notificationSnapshotReady || !notificationsEnabled
        || notificationsQuiet || !Model.hasPendingNotifications(filtered)) {
      notificationDeliveryTimer.stop()
      return
    }
    var now = Date.now()
    var remaining = Model.cooldownRemainingMs(lastNotificationAt, now, notificationCooldownMin)
    if (remaining > 0) {
      notificationDeliveryTimer.interval = remaining
      notificationDeliveryTimer.restart()
      return
    }
    var pending = filtered
    Quickshell.execDetached([
      "/usr/bin/notify-send", "--app-name", "StateSet iCommerce",
      "--urgency", pending.failedPayments > 0 ? "critical" : "normal",
      "Commerce needs attention", Model.pendingNotificationSummary(pending)
    ])
    lastNotificationAt = now
    pendingNotifications = Model.normalizeNotificationDelta({})
    scheduleNotificationStateSave()
    notificationDeliveryTimer.stop()
  }

  function refresh() {
    if (refreshing || statusProcess.running) return
    refreshTimer.stop()
    refreshing = true
    _stdout = ""
    _stderr = ""
    _stdoutTruncated = false
    _timedOut = false
    lastAttempt = new Date()
    statusProcess.running = true
  }

  function appendStdout(data) {
    var result = Model.appendBounded(_stdout, data, Model.MAX_OUTPUT_CHARS)
    _stdout = result.text
    _stdoutTruncated = _stdoutTruncated || result.truncated
  }

  function appendStderr(data) {
    _stderr = Model.appendBounded(_stderr, data, Model.MAX_ERROR_CHARS).text
  }

  function fail(messageText, kind) {
    ready = false
    failureKind = kind || "controller-error"
    lastError = Model.safeText(messageText, 240, "StateSet controller failed")
    message = lastError
    consecutiveFailures += 1
    scheduleRefresh()
  }

  function scheduleRefresh() {
    var delay = Model.retryIntervalSeconds(refreshIntervalSec, consecutiveFailures)
    nextRefreshAt = new Date(Date.now() + delay * 1000)
    refreshTimer.interval = delay * 1000
    refreshTimer.restart()
  }

  function refreshService() {
    if (serviceStatusProcess.running || actionRunning || !supportsCapability("mcp-service")) return
    _serviceOutput = ""
    _serviceTruncated = false
    mcpRefreshing = true
    serviceStatusProcess.running = true
  }

  function appendServiceOutput(data) {
    var result = Model.appendBounded(_serviceOutput, data, Model.MAX_ERROR_CHARS)
    _serviceOutput = result.text
    _serviceTruncated = _serviceTruncated || result.truncated
  }

  function finishService(exitCode) {
    mcpRefreshing = false
    if (exitCode !== 0 || _serviceTruncated) {
      mcpStatusKnown = false
      mcpActive = false
      mcpLastError = Model.safeText(_serviceOutput, 160, "Unable to read MCP service status")
      mcpState = "unknown"
      return
    }
    try {
      var value = Model.parseServiceStatusJson(_serviceOutput)
      mcpInstalled = value.installed
      mcpActive = value.active
      mcpStatusKnown = true
      mcpState = value.state
      mcpLastError = ""
      mcpLastUpdated = new Date()
    } catch (error) {
      mcpStatusKnown = false
      mcpActive = false
      mcpLastError = "Invalid MCP service response"
      mcpState = "unknown"
    }
  }

  function runServiceAction(action) {
    var command = Model.serviceActionCommand(action)
    if (actionRunning || mcpRefreshing || command.length === 0 || !supportsCapability("mcp-service")) return false
    if (!ready || !configured) {
      actionError = action === "install"
        ? "Configure a readable store before installing MCP"
        : "Refresh store status before managing MCP"
      actionStatus = actionError
      actionClearTimer.restart()
      return false
    }
    actionClearTimer.stop()
    actionKind = action
    actionStatus = action.charAt(0).toUpperCase() + action.slice(1) + "ing MCP service…"
    if (action === "stop") actionStatus = "Stopping MCP service…"
    actionError = ""
    _actionOutput = ""
    _actionTruncated = false
    _actionTimedOut = false
    actionRunning = true
    serviceActionProcess.command = command
    serviceActionProcess.running = true
    return true
  }

  function appendActionOutput(data) {
    var result = Model.appendBounded(_actionOutput, data, Model.MAX_ERROR_CHARS)
    _actionOutput = result.text
    _actionTruncated = _actionTruncated || result.truncated
  }

  function finishServiceAction(exitCode) {
    actionDeadline.stop()
    actionKillDeadline.stop()
    actionRunning = false
    var timedOut = _actionTimedOut || exitCode === 124
    var success = exitCode === 0 && !timedOut && !_actionTruncated
    if (success && actionKind !== "install") {
      try {
        var status = Model.parseServiceStatusJson(_actionOutput)
        mcpInstalled = status.installed
        mcpActive = status.active
        mcpStatusKnown = true
        mcpState = status.state
        mcpLastUpdated = new Date()
      } catch (error) {
        success = false
      }
    }
    if (success) {
      actionStatus = Model.serviceActionLabel(actionKind, true)
      actionError = ""
      actionRefreshTimer.restart()
    } else {
      actionError = timedOut ? "MCP service action timed out"
        : _actionTruncated ? "MCP service action returned too much output"
        : Model.safeText(_actionOutput, 200, Model.serviceActionLabel(actionKind, false))
      actionStatus = actionError
    }
    actionClearTimer.restart()
  }

  function refreshIfStale() {
    var updated = lastUpdated instanceof Date ? lastUpdated.getTime() : 0
    if (updated <= 0 || Date.now() - updated >= refreshIntervalSec * 1000) refresh()
  }

  function finish(exitCode) {
    refreshing = false
    statusDeadline.stop()
    killDeadline.stop()
    var failure = Model.classifyFailure(exitCode, _stderr || _stdout, _timedOut, _stdoutTruncated)
    if (failure === "timeout") {
      fail("StateSet controller timed out", failure)
      return
    }
    if (failure === "oversized-response") {
      fail("StateSet controller response exceeded 64 KiB", failure)
      return
    }
    if (exitCode !== 0) {
      fail(_stderr || _stdout || "StateSet controller is unavailable; install @stateset/cli first", failure)
      return
    }
    try {
      var value = Model.parseStatusJson(_stdout)
      ready = value.ok
      statusSchemaVersion = value.schemaVersion
      controllerVersion = value.controllerVersion
      capabilitiesKnown = value.capabilitiesKnown
      capabilities = value.capabilities
      configured = value.configured
      dbPath = value.dbPath
      mode = value.mode
      message = value.message
      sizeBytes = value.sizeBytes
      counts = value.counts
      signalsComplete = value.signalsComplete
      unavailableSignals = value.unavailableSignals
      var nextAlerts = Model.normalizeAlerts(value.alerts)
      if (value.ok) queueNotifications(alerts, nextAlerts, hasAlertBaseline)
      alerts = nextAlerts
      hasAlertBaseline = true
      hasSnapshot = true
      snapshotRestored = false
      failureKind = ready ? "" : (configured ? "store-unavailable" : "not-configured")
      lastError = ready ? "" : message
      lastUpdated = new Date()
      saveSnapshotState(value, lastUpdated.getTime())
      consecutiveFailures = ready ? 0 : consecutiveFailures + 1
      scheduleRefresh()
      refreshService()
    } catch (error) {
      var incompatible = String(error).indexOf("Unsupported status schema version") >= 0
        || String(error).indexOf("Unsupported controller version") >= 0
      fail(incompatible ? "StateSet controller uses an unsupported status contract" : "Invalid response from stateset-omarchy",
        incompatible ? "incompatible-controller" : "invalid-response")
    }
  }

  Timer {
    id: refreshTimer
    interval: root.refreshIntervalSec * 1000
    repeat: false
    running: false
    onTriggered: root.refresh()
  }

  Component.onCompleted: root.refresh()

  Timer {
    interval: 300000
    repeat: true
    running: root.hasSnapshot
    onTriggered: root.refreshService()
  }

  Timer {
    id: notificationDeliveryTimer
    interval: 1000
    repeat: false
    onTriggered: root.deliverPendingNotifications()
  }

  Timer {
    id: notificationStateSaveTimer
    interval: 200
    repeat: false
    onTriggered: root.flushNotificationState()
  }

  Timer {
    id: actionRefreshTimer
    interval: 500
    repeat: false
    onTriggered: root.refreshService()
  }

  Timer {
    id: actionClearTimer
    interval: 5000
    repeat: false
    onTriggered: {
      root.actionStatus = ""
      root.actionError = ""
      if (!root.actionRunning) root.actionKind = ""
    }
  }

  Timer {
    id: actionDeadline
    interval: 10000
    repeat: false
    onTriggered: {
      if (!serviceActionProcess.running) return
      root._actionTimedOut = true
      serviceActionProcess.signal(15)
      actionKillDeadline.restart()
    }
  }

  Timer {
    id: actionKillDeadline
    interval: 1000
    repeat: false
    onTriggered: if (serviceActionProcess.running) serviceActionProcess.signal(9)
  }

  Connections {
    target: root.notificationService
    function onDoNotDisturbChanged() {
      if (!root.notificationsQuiet) root.deliverPendingNotifications()
    }
  }

  onNotificationsEnabledChanged: {
    if (!notificationsEnabled) {
      pendingNotifications = Model.normalizeNotificationDelta({})
      scheduleNotificationStateSave()
    } else {
      deliverPendingNotifications()
    }
  }

  onSettingsChanged: deliverPendingNotifications()

  Process {
    id: notificationStateDirProcess
    command: [
      "/usr/bin/bash", "-c",
      "set -e; /usr/bin/install -d -m 700 -- \"$1\"; /usr/bin/touch -- \"$2\" \"$3\"; /usr/bin/chmod 600 -- \"$2\" \"$3\"",
      "stateset-state-init", root.notificationStateDir, root.notificationStatePath, root.snapshotStatePath
    ]
    running: true
    onExited: function(exitCode) {
      if (exitCode === 0) {
        notificationStateFile.reload()
        snapshotStateFile.reload()
      } else {
        root.loadNotificationState("")
        root.loadSnapshotState("")
      }
    }
  }

  Process {
    id: statePermissionsProcess
    command: ["/usr/bin/chmod", "600", "--", root.notificationStatePath, root.snapshotStatePath]
    running: false
  }

  Timer {
    id: statePermissionsTimer
    interval: 50
    repeat: false
    onTriggered: {
      if (statePermissionsProcess.running) restart()
      else statePermissionsProcess.running = true
    }
  }

  FileView {
    id: notificationStateFile
    path: root.notificationStatePath
    watchChanges: false
    atomicWrites: true
    printErrors: false
    onLoaded: root.loadNotificationState(text())
    onLoadFailed: root.loadNotificationState("")
  }

  FileView {
    id: snapshotStateFile
    path: root.snapshotStatePath
    watchChanges: false
    atomicWrites: true
    printErrors: false
    onLoaded: root.loadSnapshotState(text())
    onLoadFailed: root.loadSnapshotState("")
  }

  Timer {
    id: statusDeadline
    interval: 10000
    repeat: false
    onTriggered: {
      if (!statusProcess.running) return
      root._timedOut = true
      statusProcess.running = false
      killDeadline.restart()
    }
  }

  Timer {
    id: killDeadline
    interval: 1000
    repeat: false
    onTriggered: statusProcess.signal(9)
  }

  Process {
    id: statusProcess
    running: false
    command: ["/usr/bin/bash", "-c", "set -o pipefail; { /usr/bin/timeout --signal=TERM --kill-after=1s 8s stateset-omarchy status --json; } 2>&1 | /usr/bin/head -c 65537"]
    stdout: SplitParser {
      splitMarker: ""
      onRead: function(data) { root.appendStdout(data) }
    }
    stderr: SplitParser {
      splitMarker: ""
      onRead: function(data) { root.appendStderr(data) }
    }
    onStarted: statusDeadline.restart()
    onExited: function(exitCode) {
      root.finish(exitCode)
    }
  }


  Process {
    id: serviceStatusProcess
    running: false
    command: ["/usr/bin/bash", "-c", "set -o pipefail; { /usr/bin/timeout --signal=TERM --kill-after=1s 5s stateset-omarchy service status --json; } 2>&1 | /usr/bin/head -c 4097"]
    stdout: SplitParser {
      splitMarker: ""
      onRead: function(data) { root.appendServiceOutput(data) }
    }
    stderr: SplitParser {
      splitMarker: ""
      onRead: function(data) { root.appendServiceOutput(data) }
    }
    onExited: function(exitCode) { root.finishService(exitCode) }
  }

  Process {
    id: serviceActionProcess
    running: false
    command: []
    stdout: SplitParser {
      splitMarker: ""
      onRead: function(data) { root.appendActionOutput(data) }
    }
    stderr: SplitParser {
      splitMarker: ""
      onRead: function(data) { root.appendActionOutput(data) }
    }
    onStarted: actionDeadline.restart()
    onExited: function(exitCode) { root.finishServiceAction(exitCode) }
  }
}
