import QtQuick
import Quickshell
import Quickshell.Io
import "Model.js" as Model

Item {
  id: root

  property var settings: ({})
  property bool refreshing: false
  property bool ready: false
  property bool configured: false
  property string dbPath: ""
  property string mode: "preview"
  property string message: "Discovering store"
  property string lastError: ""
  property var counts: ({ orders: 0, customers: 0, products: 0, returns: 0, payments: 0 })
  property var alerts: ({ pendingOrders: 0, failedPayments: 0, pendingReturns: 0, lowStock: 0, total: 0 })
  property bool hasAlertBaseline: false
  property date lastUpdated: new Date(0)
  property string _output: ""
  property bool _truncated: false
  property bool _timedOut: false

  readonly property int refreshIntervalSec: {
    var value = parseInt(String(settings && settings.refreshIntervalSec || 120), 10)
    if (!isFinite(value)) value = 120
    return Math.max(30, Math.min(1800, value))
  }
  readonly property bool notificationsEnabled: settings && settings.notifications !== false

  function refresh() {
    if (refreshing || statusProcess.running) return
    refreshing = true
    _output = ""
    _truncated = false
    _timedOut = false
    statusProcess.running = true
  }

  function appendOutput(data) {
    var result = Model.appendBounded(_output, String(data || ""), Model.MAX_OUTPUT_CHARS)
    _output = result.text
    _truncated = _truncated || result.truncated
  }

  function fail(messageText) {
    ready = false
    configured = false
    lastError = Model.safeText(messageText, 240, "StateSet controller failed")
    message = lastError
  }

  function refreshIfStale() {
    var updated = lastUpdated instanceof Date ? lastUpdated.getTime() : 0
    if (updated <= 0 || Date.now() - updated >= refreshIntervalSec * 1000) refresh()
  }

  function finish(exitCode) {
    refreshing = false
    statusDeadline.stop()
    killDeadline.stop()
    if (_timedOut) {
      fail("StateSet controller timed out")
      return
    }
    if (_truncated) {
      fail("StateSet controller response exceeded 64 KiB")
      return
    }
    if (exitCode !== 0) {
      fail(_output || "StateSet controller is unavailable; install @stateset/cli first")
      return
    }
    try {
      var value = Model.parseStatusJson(_output)
      ready = value.ok
      configured = value.configured
      dbPath = value.dbPath
      mode = value.mode
      message = value.message
      counts = value.counts
      var nextAlerts = Model.normalizeAlerts(value.alerts)
      if (notificationsEnabled && Model.shouldNotify(alerts, nextAlerts, hasAlertBaseline)) {
        Quickshell.execDetached([
          "notify-send", "--app-name", "StateSet iCommerce", "--urgency", nextAlerts.failedPayments > alerts.failedPayments ? "critical" : "normal",
          "Commerce needs attention", Model.attentionSummary(nextAlerts)
        ])
      }
      alerts = nextAlerts
      hasAlertBaseline = true
      lastError = ""
      lastUpdated = new Date()
    } catch (error) {
      fail("Invalid response from stateset-omarchy")
    }
  }

  Timer {
    interval: root.refreshIntervalSec * 1000
    repeat: true
    running: true
    triggeredOnStart: true
    onTriggered: root.refresh()
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
      onRead: function(data) { root.appendOutput(data) }
    }
    stderr: SplitParser {
      splitMarker: ""
      onRead: function(data) { root.appendOutput(data) }
    }
    onStarted: statusDeadline.restart()
    onExited: function(exitCode) {
      root.finish(exitCode)
    }
  }
}
