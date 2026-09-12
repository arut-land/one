pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import dev.arut.bindings 1.0
import dev.arut.linux 1.0

ApplicationWindow {
    id: window
    visible: true
    width: 900
    height: 560
    minimumWidth: 360
    minimumHeight: 420
    title: "Arut"
    property bool sidebarExpanded: true

    required property ChatModel chat

    RowLayout {
        anchors.fill: parent
        spacing: 0

        Pane {
            visible: window.width >= 620
            Layout.preferredWidth: window.sidebarExpanded ? 236 : 64
            Layout.fillHeight: true
            padding: window.sidebarExpanded ? 12 : 8

            ColumnLayout {
                anchors.fill: parent
                spacing: 8

                RowLayout {
                    Layout.fillWidth: true
                    Label {
                        Layout.fillWidth: true
                        text: window.sidebarExpanded ? "Arut" : "A"
                        horizontalAlignment: window.sidebarExpanded ? Text.AlignLeft : Text.AlignHCenter
                        font.pixelSize: 22
                        font.bold: true
                    }
                    ToolButton {
                        visible: window.sidebarExpanded
                        icon.name: "go-previous"
                        Accessible.name: "Collapse chat history"
                        onClicked: window.sidebarExpanded = false
                    }
                }
                ToolButton {
                    visible: !window.sidebarExpanded
                    Layout.alignment: Qt.AlignHCenter
                    icon.name: "go-next"
                    Accessible.name: "Expand chat history"
                    onClicked: window.sidebarExpanded = true
                }
                Button {
                    Layout.fillWidth: true
                    text: window.sidebarExpanded ? "New chat" : ""
                    icon.name: "document-new"
                    Accessible.name: "New chat"
                    onClicked: {
                        chat.newChat()
                        composer.forceActiveFocus()
                    }
                }
                Label {
                    visible: window.sidebarExpanded
                    text: "RECENT"
                    color: palette.mid
                    font.pixelSize: 11
                    font.letterSpacing: 1.2
                    Layout.leftMargin: 8
                    Layout.topMargin: 12
                }
                ListView {
                    visible: window.sidebarExpanded
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 4
                    model: chat.historyTitles
                    delegate: ItemDelegate {
                        required property string modelData
                        required property int index
                        width: ListView.view.width
                        text: modelData
                        highlighted: chat.selectedHistoryIndex === index
                        Accessible.name: "Open chat " + modelData
                        onClicked: {
                            chat.selectChat(index)
                            composer.forceActiveFocus()
                        }
                    }
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

        ToolBar {
            Layout.fillWidth: true
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 16
                anchors.rightMargin: 16
                Label {
                    Layout.fillWidth: true
                    text: chat.selectedHistoryIndex >= 0
                        ? chat.historyTitles[chat.selectedHistoryIndex]
                        : "New conversation"
                    font.bold: true
                }
                Label {
                    visible: chat.status === ChatModel.Sending
                    text: "Thinking..."
                    color: palette.mid
                    font.pixelSize: 12
                }
                ToolButton {
                    visible: window.width < 620
                    text: "Chats"
                    icon.name: "view-list"
                    Accessible.name: "Open chat history"
                    onClicked: historyDrawer.open()
                }
                ToolButton {
                    icon.name: "document-new"
                    Accessible.name: "New chat"
                    onClicked: {
                        chat.newChat()
                        composer.forceActiveFocus()
                    }
                }
            }
        }

        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            Column {
                anchors.centerIn: parent
                spacing: 8
                visible: chat.messageTexts.length === 0
                Label {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "A"
                    font.pixelSize: 28
                    font.bold: true
                }
                Label {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "What are we working on?"
                    font.pixelSize: 20
                    font.bold: true
                }
                Label {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: "Your draft stays with this conversation."
                    color: palette.mid
                }
            }

            ListView {
                id: messageList
                anchors.fill: parent
                anchors.margins: 20
                clip: true
                spacing: 12
                model: chat.messageTexts
                ScrollBar.vertical: ScrollBar {}
                onCountChanged: positionViewAtEnd()

                delegate: Item {
                    id: messageDelegate
                    required property string modelData
                    required property int index
                    readonly property bool isUser: window.chat.messageRoles[index] === "user"
                    width: ListView.view.width
                    height: bubble.height + 8

                    Rectangle {
                        id: bubble
                        width: Math.min(messageText.implicitWidth + 30, parent.width * 0.78)
                        height: messageText.implicitHeight + 22
                        anchors.right: parent.isUser ? parent.right : undefined
                        anchors.left: parent.isUser ? undefined : parent.left
                        radius: 14
                        color: parent.isUser ? palette.highlight : palette.alternateBase
                        border.width: parent.isUser ? 0 : 1
                        border.color: palette.midlight

                        Label {
                            id: messageText
                            anchors.centerIn: parent
                            width: Math.min(implicitWidth, messageList.width * 0.78 - 30)
                            text: messageDelegate.modelData
                            color: messageDelegate.isUser ? palette.highlightedText : palette.text
                            wrapMode: Text.Wrap
                            textFormat: Text.PlainText
                        }
                    }
                }
            }
        }

        Label {
            Layout.fillWidth: true
            Layout.leftMargin: 20
            Layout.rightMargin: 20
            visible: chat.error.length > 0
            text: chat.error
            color: "#b42318"
            wrapMode: Text.WordWrap
        }

        Pane {
            Layout.fillWidth: true
            padding: 12
            RowLayout {
                width: parent.width
                TextField {
                    id: composer
                    Layout.fillWidth: true
                    placeholderText: "Message Arut"
                    text: chat.draft
                    focus: true
                    enabled: chat.status !== ChatModel.Sending
                    onTextEdited: chat.replaceDraft(text)
                    onAccepted: sendMessage()

                    function sendMessage() {
                        const message = text.trim()
                        if (message.length === 0)
                            return
                        chat.send(message)
                    }
                }
                Button {
                    text: chat.status === ChatModel.Sending ? "Sending..." : "Send"
                    enabled: composer.text.trim().length > 0
                        && chat.status !== ChatModel.Sending
                    onClicked: composer.sendMessage()
                }
            }
        }
        }
    }

    Popup {
        id: historyDrawer
        x: 0
        y: 0
        width: Math.min(window.width * 0.84, 300)
        height: window.height
        modal: true
        dim: true
        padding: 12
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

        contentItem: ColumnLayout {
            spacing: 8
            RowLayout {
                Layout.fillWidth: true
                Label {
                    Layout.fillWidth: true
                    text: "Arut"
                    font.pixelSize: 22
                    font.bold: true
                }
                ToolButton {
                    icon.name: "window-close"
                    Accessible.name: "Close chat history"
                    onClicked: historyDrawer.close()
                }
            }
            Button {
                Layout.fillWidth: true
                text: "New chat"
                icon.name: "document-new"
                onClicked: {
                    chat.newChat()
                    historyDrawer.close()
                    composer.forceActiveFocus()
                }
            }
            Label {
                text: "RECENT"
                color: palette.mid
                font.pixelSize: 11
                font.letterSpacing: 1.2
            }
            ListView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 4
                model: chat.historyTitles
                delegate: ItemDelegate {
                    required property string modelData
                    required property int index
                    width: ListView.view.width
                    text: modelData
                    highlighted: chat.selectedHistoryIndex === index
                    onClicked: {
                        chat.selectChat(index)
                        historyDrawer.close()
                        composer.forceActiveFocus()
                    }
                }
            }
        }
    }

    Shortcut { sequence: "Ctrl+N"; onActivated: chat.newChat() }
    Shortcut { sequence: "Ctrl+B"; onActivated: window.sidebarExpanded = !window.sidebarExpanded }
    Shortcut { sequence: "Ctrl+K"; onActivated: composer.forceActiveFocus() }
}
