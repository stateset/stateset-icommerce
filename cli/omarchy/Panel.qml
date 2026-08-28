import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "ServiceHost.js" as ServiceHost

Panel {
  id: root
  moduleName: "com.stateset.icommerce"
  ipcTarget: "com.stateset.icommerce"
  manageIpc: false

  QtObject {
    id: fallbackService
    property var settings: ({})
    property bool refreshing: false
    property bool ready: false
    property bool configured: false
    property string dbPath: ""
    property string mode: "preview"
    property string message: "Starting StateSet"
    property string lastError: ""
    property var counts: ({ orders: 0, customers: 0, products: 0, returns: 0, payments: 0 })
    property var alerts: ({ pendingOrders: 0, failedPayments: 0, pendingReturns: 0, lowStock: 0, total: 0 })
    function refresh() {}
    function refreshIfStale() {}
  }

  readonly property var hostedService: {
    var _ = bar && bar.shell ? bar.shell._services : null
    return ServiceHost.hostedService(bar)
  }
  readonly property var service: hostedService !== null ? hostedService : fallbackService
  readonly property color foreground: bar ? bar.foreground : Color.foreground
  readonly property color urgent: bar ? bar.urgent : Color.urgent
  readonly property color dim: Qt.darker(foreground, 1.55)
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family
  readonly property var allowedCommands: ["agent", "attention", "backup", "dashboard", "remediate"]

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  function launch(command) {
    if (!bar || allowedCommands.indexOf(command) < 0) return
    var controller = "if command -v stateset-omarchy >/dev/null 2>&1; then stateset-omarchy " + command
      + "; else npx -y -p @stateset/cli stateset-omarchy " + command + "; fi"
    bar.run("omarchy-launch-floating-terminal-with-presentation '" + controller + "'")
    close()
  }

  onOpenedChanged: if (opened) service.refreshIfStale()

  Binding {
    target: root.hostedService
    property: "settings"
    value: root.settings
    when: root.hostedService !== null
  }

  IpcHandler {
    target: root.ipcTarget
    function open(): void { root.open() }
    function close(): void { root.close() }
    function toggle(): void { root.toggle() }
    function refresh(): string { service.refresh(); return "ok" }
    function status(): string {
      return JSON.stringify({ ready: service.ready, mode: service.mode, dbPath: service.dbPath, counts: service.counts, alerts: service.alerts, error: service.lastError })
    }
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: !service.ready ? "󰅖" : (service.alerts.total || 0) > 0 ? "󰀪" : "󰆼"
    foreground: !service.ready || (service.alerts.failedPayments || 0) > 0 ? root.urgent : root.foreground
    tooltipText: service.refreshing ? "Refreshing StateSet Commerce" : service.message
    onPressed: function(buttonCode) {
      if (buttonCode === Qt.RightButton || buttonCode === Qt.MiddleButton) service.refresh()
      else root.toggle()
    }
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(390))
    contentHeight: panel.fittedContentHeight(content.implicitHeight, Style.space(520))

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }
      onTextKey: function(text) { if (text === "r" || text === "R") service.refresh() }

      ColumnLayout {
        id: content
        anchors.fill: parent
        spacing: Style.space(12)

        RowLayout {
          Layout.fillWidth: true
          spacing: Style.space(10)

          Text {
            text: "󰆼"
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.display
          }
          ColumnLayout {
            Layout.fillWidth: true
            spacing: Style.space(2)
            Text {
              text: "StateSet Commerce"
              color: root.foreground
              font.family: root.fontFamily
              font.pixelSize: Style.font.title
              font.bold: true
            }
            Text {
              Layout.fillWidth: true
              text: service.message.toUpperCase()
              color: service.ready ? root.dim : root.urgent
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
              elide: Text.ElideMiddle
            }
          }
          PanelActionButton {
            iconText: service.refreshing ? "󰑓" : "󰑐"
            foreground: root.foreground
            fontFamily: root.fontFamily
            enabled: !service.refreshing
            onClicked: service.refresh()
          }
        }

        PanelSeparator { foreground: root.foreground }

        Text {
          Layout.fillWidth: true
          text: service.dbPath || "Run stateset-omarchy install inside a store project"
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
          elide: Text.ElideMiddle
        }

        Rectangle {
          visible: (service.alerts.total || 0) > 0
          Layout.fillWidth: true
          implicitHeight: attentionRow.implicitHeight + Style.space(18)
          radius: Style.cornerRadius
          color: Color.popups.background
          border.color: (service.alerts.failedPayments || 0) > 0 ? root.urgent : Color.accent
          border.width: 1

          RowLayout {
            id: attentionRow
            anchors.fill: parent
            anchors.margins: Style.space(9)
            spacing: Style.space(8)
            Text {
              text: (service.alerts.failedPayments || 0) > 0 ? "󰅖" : "󰀪"
              color: (service.alerts.failedPayments || 0) > 0 ? root.urgent : Color.accent
              font.family: root.fontFamily
              font.pixelSize: Style.font.title
            }
            ColumnLayout {
              Layout.fillWidth: true
              spacing: 0
              Text {
                text: (service.alerts.total || 0) + " NEED ATTENTION"
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.body
                font.bold: true
              }
              Text {
                Layout.fillWidth: true
                text: [
                  (service.alerts.failedPayments || 0) > 0 ? service.alerts.failedPayments + " failed payments" : "",
                  (service.alerts.lowStock || 0) > 0 ? service.alerts.lowStock + " low stock" : "",
                  (service.alerts.pendingReturns || 0) > 0 ? service.alerts.pendingReturns + " returns" : "",
                  (service.alerts.pendingOrders || 0) > 0 ? service.alerts.pendingOrders + " orders" : ""
                ].filter(function(value) { return value !== "" }).join(" · ")
                color: root.dim
                font.family: root.fontFamily
                font.pixelSize: Style.font.caption
                elide: Text.ElideRight
              }
            }
            ColumnLayout {
              spacing: Style.space(4)
              Button {
                text: "REVIEW"
                bordered: true
                foreground: root.foreground
                background: Color.popups.background
                accent: Color.accent
                fontFamily: root.fontFamily
                onClicked: root.launch("attention")
              }
              Button {
                text: "RESOLVE"
                bordered: true
                foreground: root.foreground
                background: Color.popups.background
                accent: root.urgent
                fontFamily: root.fontFamily
                onClicked: root.launch("remediate")
              }
            }
          }
        }

        Rectangle {
          Layout.fillWidth: true
          implicitHeight: metrics.implicitHeight + Style.space(18)
          radius: Style.cornerRadius
          color: Color.popups.background
          border.color: service.mode === "preview" ? Color.accent : root.urgent
          border.width: 1
          RowLayout {
            id: metrics
            anchors.fill: parent
            anchors.margins: Style.space(9)
            Repeater {
              model: [
                { label: "ORDERS", value: service.counts.orders || 0 },
                { label: "CUSTOMERS", value: service.counts.customers || 0 },
                { label: "PRODUCTS", value: service.counts.products || 0 },
                { label: "RETURNS", value: service.counts.returns || 0 }
              ]
              ColumnLayout {
                id: metric
                required property var modelData
                Layout.fillWidth: true
                spacing: 0
                Text {
                  Layout.alignment: Qt.AlignHCenter
                  text: metric.modelData.value
                  color: root.foreground
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.title
                  font.bold: true
                }
                Text {
                  Layout.alignment: Qt.AlignHCenter
                  text: metric.modelData.label
                  color: root.dim
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.caption
                }
              }
            }
          }
        }

        Text {
          Layout.fillWidth: true
          text: service.mode === "preview" ? "PREVIEW ONLY · WRITES REQUIRE EXPLICIT APPLY" : "GOVERNED APPLY · OPERATOR POLICY ACTIVE"
          color: service.mode === "preview" ? Color.accent : root.urgent
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          horizontalAlignment: Text.AlignHCenter
          font.bold: true
        }

        RowLayout {
          Layout.fillWidth: true
          spacing: Style.space(8)
          Button {
            Layout.fillWidth: true
            text: "DASHBOARD"
            bordered: true
            foreground: root.foreground
            background: Color.popups.background
            accent: Color.accent
            fontFamily: root.fontFamily
            onClicked: root.launch("dashboard")
          }
          Button {
            Layout.fillWidth: true
            text: "AGENT"
            bordered: true
            foreground: root.foreground
            background: Color.popups.background
            accent: Color.accent
            fontFamily: root.fontFamily
            onClicked: root.launch("agent")
          }
          Button {
            Layout.fillWidth: true
            text: "BACKUP"
            bordered: true
            foreground: root.foreground
            background: Color.popups.background
            accent: Color.accent
            fontFamily: root.fontFamily
            onClicked: root.launch("backup")
          }
        }
      }
    }
  }
}
