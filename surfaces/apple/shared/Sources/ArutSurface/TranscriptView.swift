import ArutBindings
import SwiftUI
#if os(macOS)
import AppKit
#else
import UIKit
#endif

struct TranscriptView: View {
    let messages: [ChatMessage]
    let scrollRequest: Int
    @Binding var readingPosition: UInt64?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var nearBottom = true

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(messages.indices, id: \.self) { index in
                        let message = messages[index]
                        VStack {
                            if message.startsTimeGroup, message.acceptedAtMs > 0 {
                                Text(Date(timeIntervalSince1970: Double(message.acceptedAtMs) / 1_000),
                                     format: .dateTime.month(.abbreviated).day().hour().minute())
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                    .padding(.vertical)
                            }
                            MessageRow(message: message)
                        }
                        .padding(.bottom, endsSpeakerGroup(at: index) ? 12 : 4)
                        .id(message.id)
                        .transition(.opacity)
                    }
                    Color.clear.frame(height: 1)
                        .id(UInt64(0))
                        .modifier(TranscriptBottomVisibility(isVisible: $nearBottom))
                }
                .modifier(TranscriptTargets())
                .frame(maxWidth: ConversationLayout.maximumWidth)
                .padding()
                .frame(maxWidth: .infinity)
                .animation(reduceMotion ? nil : .easeInOut(duration: 0.2), value: messages.count)
            }
            .modifier(TranscriptPosition(position: $readingPosition))
            .accessibilityLabel(L10n.labelTranscript())
            .accessibilityIdentifier("conversation-transcript")
            .overlay {
                if messages.isEmpty { emptyConversation.allowsHitTesting(false) }
            }
            .onAppear { proxy.scrollTo(readingPosition ?? 0, anchor: readingPosition == nil ? .bottom : .top) }
            .onChange(of: messages.last?.id) { _ in
                if nearBottom { scrollToLatest(proxy) }
            }
            .onChange(of: scrollRequest) { _ in scrollToLatest(proxy) }
            .overlay(alignment: .bottomTrailing) {
                if !nearBottom, !messages.isEmpty {
                    HStack {
                        Spacer()
                        latestButton(proxy)
                    }
                    .frame(maxWidth: ConversationLayout.maximumWidth)
                    .padding()
                    .frame(maxWidth: .infinity)
                }
            }
        }
    }

    private func endsSpeakerGroup(at index: Int) -> Bool {
        index == messages.count - 1 || messages[index + 1].startsSpeakerGroup
    }

    private func scrollToLatest(_ proxy: ScrollViewProxy) {
        withAnimation(reduceMotion ? nil : .spring(response: 0.36, dampingFraction: 0.92)) {
            proxy.scrollTo(UInt64(0), anchor: .bottom)
        }
    }

    @ViewBuilder
    private func latestButton(_ proxy: ScrollViewProxy) -> some View {
        let button = Button { scrollToLatest(proxy) } label: {
            Label(L10n.actionScrollToLatest(), systemImage: "arrow.down")
                .labelStyle(.iconOnly)
        }
        .help(L10n.actionScrollToLatest())
        if #available(macOS 26.0, iOS 26.0, *) {
            button.buttonStyle(.glass)
        } else {
            button.buttonStyle(.bordered)
        }
    }

    @ViewBuilder
    private var emptyConversation: some View {
        if #available(macOS 14.0, iOS 17.0, *) {
            ContentUnavailableView(L10n.chatEmptyTitle(), systemImage: "bubble.left.and.bubble.right",
                                   description: Text(L10n.chatEmptyHint()))
        } else {
            VStack {
                Label(L10n.chatEmptyTitle(), systemImage: "bubble.left.and.bubble.right")
                    .font(.title2)
                Text(L10n.chatEmptyHint()).foregroundStyle(.secondary)
            }
            .padding()
        }
    }
}

private struct TranscriptBottomVisibility: ViewModifier {
    @Binding var isVisible: Bool
    @ViewBuilder func body(content: Content) -> some View {
        if #available(macOS 15.0, iOS 18.0, *) {
            content.onScrollVisibilityChange(threshold: 0.1) { isVisible = $0 }
        } else {
            content.onAppear { isVisible = true }.onDisappear { isVisible = false }
        }
    }
}

private struct TranscriptTargets: ViewModifier {
    @ViewBuilder func body(content: Content) -> some View {
        if #available(macOS 14.0, iOS 17.0, *) { content.scrollTargetLayout() }
        else { content }
    }
}

private struct TranscriptPosition: ViewModifier {
    @Binding var position: UInt64?
    @ViewBuilder func body(content: Content) -> some View {
        if #available(macOS 14.0, iOS 17.0, *) { content.scrollPosition(id: $position, anchor: .top) }
        else { content }
    }
}

private struct MessageRow: View {
    let message: ChatMessage
    @Environment(\.colorSchemeContrast) private var contrast

    private var isUser: Bool { message.role == .user }
    private var time: String {
        guard message.acceptedAtMs > 0 else { return "" }
        return Date(timeIntervalSince1970: Double(message.acceptedAtMs) / 1_000)
            .formatted(date: .abbreviated, time: .shortened)
    }

    var body: some View {
        HStack {
            if isUser { Spacer() }
            Text(message.text)
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
                .foregroundStyle(isUser ? Color.white : Color.primary)
                .padding(.horizontal)
                .padding(.vertical, 10)
                .background {
                    RoundedRectangle(cornerRadius: 18)
                        .fill(isUser ? Color.blue : Color.primary.opacity(contrast == .increased ? 0.16 : 0.07))
                }
                .frame(maxWidth: 560, alignment: isUser ? .trailing : .leading)
                .help(time)
                .accessibilityLabel(isUser ? L10n.chatRoleYou() : L10n.chatRoleAssistant())
                .accessibilityValue(message.text)
                .accessibilityIdentifier("message-content-\(message.id)")
                .contextMenu {
                    Button(action: copy) { Label(L10n.actionCopyMessage(), systemImage: "doc.on.doc") }
                }
            if !isUser { Spacer() }
        }
    }

    private func copy() {
        #if os(macOS)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(message.text, forType: .string)
        #else
        UIPasteboard.general.string = message.text
        #endif
    }
}
