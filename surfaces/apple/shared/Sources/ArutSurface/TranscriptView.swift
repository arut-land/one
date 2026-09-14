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
                    // Identity comes from the row's own key; the grouping
                    // decision comes from Rust, so no index is needed here.
                    ForEach(messages, id: \.id) { message in
                        VStack {
                            if let stamp = acceptedAt(message.acceptedAtMs), message.startsTimeGroup {
                                Text(verbatim: timeGroupLabel(for: message, at: stamp))
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .padding(.vertical)
                            }
                            MessageRow(message: message)
                        }
                        .padding(.bottom, message.endsSpeakerGroup ? 12 : 4)
                        .id(message.id)
                        .transition(.opacity)
                    }
                    bottomSentinel
                }
                .scrollTargetLayout()
                .frame(maxWidth: ConversationLayout.maximumWidth)
                .padding()
                .frame(maxWidth: .infinity)
                .animation(reduceMotion ? nil : .easeInOut(duration: 0.2), value: messages.count)
            }
            .scrollPosition(id: $readingPosition, anchor: .top)
            .accessibilityLabel(L10n.labelTranscript())
            .accessibilityIdentifier("conversation-transcript")
            .onAppear {
                if let position = readingPosition {
                    proxy.scrollTo(position, anchor: .top)
                } else {
                    proxy.scrollTo(TranscriptView.bottomSentinelId, anchor: .bottom)
                }
            }
            .onChange(of: messages.last?.id) {
                announceLatest()
                if nearBottom { scrollToLatest(proxy) }
            }
            .onChange(of: scrollRequest) { scrollToLatest(proxy) }
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

    /// Outside the message-id namespace `scrollPosition(id:)` reads, so the
    /// sentinel cannot collide with a row however the core keys its rows.
    private static let bottomSentinelId = "transcript-bottom"

    private func timeGroupLabel(
        for message: ChatMessage,
        at stamp: Date
    ) -> String {
        guard let previousMilliseconds = message.previousTimeGroupAtMs,
              let previous = acceptedAt(previousMilliseconds),
              Calendar.current.isDate(previous, inSameDayAs: stamp)
        else {
            return stamp.formatted(date: .abbreviated, time: .shortened)
        }
        return stamp.formatted(date: .omitted, time: .shortened)
    }

    // Scroll visibility arrives on macOS 15/iOS 18; before that the sentinel's
    // appearance is the only signal that the reader is at the bottom.
    @ViewBuilder
    private var bottomSentinel: some View {
        let sentinel = Color.clear.frame(height: 1).id(TranscriptView.bottomSentinelId)
        if #available(macOS 15.0, iOS 18.0, *) {
            sentinel.onScrollVisibilityChange(threshold: 0.1) { nearBottom = $0 }
        } else {
            sentinel.onAppear { nearBottom = true }.onDisappear { nearBottom = false }
        }
    }

    /// VoiceOver hears an incoming reply the way it hears one on every other
    /// surface; an outgoing message was just typed, so it is not announced.
    private func announceLatest() {
        guard let message = messages.last, message.role != .user else { return }
        AccessibilityNotification
            .Announcement("\(L10n.chatRoleAssistant()): \(message.text)")
            .post()
    }

    private func scrollToLatest(_ proxy: ScrollViewProxy) {
        withAnimation(reduceMotion ? nil : .spring(response: 0.36, dampingFraction: 0.92)) {
            proxy.scrollTo(TranscriptView.bottomSentinelId, anchor: .bottom)
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
}

private struct MessageRow: View {
    let message: ChatMessage
    @Environment(\.colorSchemeContrast) private var contrast
    /// The bubble's reading width grows with the text, so an accessibility text
    /// size still gets a full line rather than a column of two or three words.
    @ScaledMetric(relativeTo: .body) private var maximumWidth: CGFloat = 560

    private var isUser: Bool { message.role == .user }
    private var time: String {
        guard let stamp = acceptedAt(message.acceptedAtMs) else { return "" }
        return stamp.formatted(date: .abbreviated, time: .shortened)
    }

    var body: some View {
        HStack {
            if isUser { Spacer() }
            Text(message.text)
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
                // The bubble takes the accent the person chose, not a literal
                // blue. Apple publishes no "on accent" colour, and Messages
                // reads white over the filled bubble, so white it is.
                .foregroundStyle(isUser ? AnyShapeStyle(.white) : AnyShapeStyle(Color.primary))
                .padding(.horizontal)
                .padding(.vertical, 10)
                .background {
                    RoundedRectangle(cornerRadius: 18)
                        .fill(
                            isUser
                                ? AnyShapeStyle(Color.accentColor)
                                : AnyShapeStyle(
                                    Color.primary.opacity(contrast == .increased ? 0.16 : 0.07)
                                )
                        )
                }
                .frame(maxWidth: maximumWidth, alignment: isUser ? .trailing : .leading)
                .help(time)
                .accessibilityLabel(isUser ? L10n.chatRoleYou() : L10n.chatRoleAssistant())
                .accessibilityValue(message.text)
                .accessibilityIdentifier("message-content-\(message.id)")
                .contextMenu {
                    Button(action: copy) {
                        Label(L10n.actionCopyMessage(), systemImage: "doc.on.doc")
                    }
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
