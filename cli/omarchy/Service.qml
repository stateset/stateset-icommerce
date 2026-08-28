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
  property string _stdout: ""
  property string _stderr: ""

  readonly property int refreshIntervalSec: {
    var value = parseInt(String(settings && settings.refreshIntervalSec || 120), 10)
    if (!isFinite(value)) value = 120
    return Math.max(30, Math.min(1800, value))
  }
  readonly property bool notificationsEnabled: settings && settings.notifications !== false

  function refresh() {
    if (refreshing || statusProcess.running) return
    refreshing = true
    _stdout = ""
    _stderr = ""
    statusProcess.running = true
  }

  function refreshIfStale() {
    var updated = lastUpdated instanceof Date ? lastUpdated.getTime() : 0
    if (updated <= 0 || Date.now() - updated >= refreshIntervalSec * 1000) refresh()
  }

  function finish(exitCode) {
    refreshing = false
    if (exitCode !== 0) {
      ready = false
      lastError = String(_stderr || _stdout || "StateSet status failed").replace(/\s+/g, " ").trim()
      message = lastError
      return
    }
    try {
      var value = JSON.parse(String(_stdout || "{}"))
      ready = value.ok === true
      configured = value.configured === true
      dbPath = String(value.dbPath || "")
      mode = String(value.mode || "preview")
      message = String(value.message || (ready ? "Store ready" : "Store unavailable"))
      counts = value.counts || counts
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
      ready = false
      lastError = "Invalid response from stateset-omarchy"
      message = lastError
    }
  }

  Timer {
    interval: root.refreshIntervalSec * 1000
    repeat: true
    running: true
    triggeredOnStart: true
    onTriggered: root.refresh()
  }

  Process {
    id: statusProcess
    running: false
    command: ["bash", "-lc", "if command -v stateset-omarchy >/dev/null 2>&1; then exec stateset-omarchy status --json; else exec npx -y -p @stateset/cli stateset-omarchy status --json; fi"]
    stdout: StdioCollector {
      id: statusStdout
      waitForEnd: true
      onStreamFinished: root._stdout = text
    }
    stderr: StdioCollector {
      id: statusStderr
      waitForEnd: true
      onStreamFinished: root._stderr = text
    }
    onExited: function(exitCode) {
      root._stdout = String(statusStdout.text || root._stdout || "")
      root._stderr = String(statusStderr.text || root._stderr || "")
      root.finish(exitCode)
    }
  }
}
