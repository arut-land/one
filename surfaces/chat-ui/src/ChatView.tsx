import {
  Fragment,
  createContext,
  use,
  useEffect,
  useRef,
  useState,
  type ReactNode,
  type TouchEvent,
} from "react";
import type { ChatMessage, ChatSummary, MatchRange } from "@arut/bindings-typescript";
import { acceptedAt } from "@arut/bindings-typescript/observable";
import { t, type L10nBundle } from "@arut/bindings-typescript/strings";
import { userRole } from "./port";
import type { useChat } from "./useChat";

/** The negotiated strings, supplied once by the composition root. */
export const Strings = createContext<L10nBundle>({ format: (key: string) => key });

type ChatViewProps = ReturnType<typeof useChat>;

const spinner = <span className="spinner" />;

/**
 * The words around a time group. Rust decides *where* a group starts
 * (`startsTimeGroup`) and hands over the instant; the platform decides how an
 * instant reads, which is the one half of a timestamp that is never shared.
 */
const stamp = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

export function ChatView(props: ChatViewProps) {
  const strings = use(Strings);
  const [sidebarOpen, setSidebarOpen] = useState(
    () => globalThis.matchMedia?.("(min-width: 760px)").matches ?? true,
  );
  const touchStart = useRef(0);
  const composer = useRef<HTMLTextAreaElement>(null);
  const announcer = useRef<HTMLParagraphElement>(null);
  const announced = useRef({ chat: "", id: 0n });

  useEffect(() => {
    const desktop = globalThis.matchMedia?.("(min-width: 760px)");
    const adapt = (event: MediaQueryListEvent) => setSidebarOpen(event.matches);
    const shortcut = (event: KeyboardEvent) => {
      if (event.key === "Escape") setSidebarOpen(false);
      // A chat is text the whole way down, so a bare letter is never a global
      // shortcut here: only the modified one reaches the composer.
      if ((event.ctrlKey || event.metaKey) && !event.altKey && event.key.toLowerCase() === "l") {
        event.preventDefault();
        composer.current?.focus();
      }
    };
    desktop?.addEventListener("change", adapt);
    globalThis.addEventListener("keydown", shortcut);
    return () => {
      desktop?.removeEventListener("change", adapt);
      globalThis.removeEventListener("keydown", shortcut);
    };
  }, []);

  // A draft is multi-line, so the field grows with it up to the height the
  // stylesheet caps, after which it scrolls.
  useEffect(() => {
    const area = composer.current;
    if (area === null) return;
    area.style.height = "auto";
    area.style.height = `${area.scrollHeight}px`;
  }, [props.draft]);

  // One announcement per arriving reply, written straight into a region React
  // renders empty: opening a conversation adopts its tail silently, so
  // switching conversations never reads a whole transcript back out.
  useEffect(() => {
    const region = announcer.current;
    if (region === null) return;
    const latest = props.messages.at(-1);
    const mark = announced.current;
    if (mark.chat !== props.chatKey) {
      announced.current = { chat: props.chatKey, id: latest?.id ?? 0n };
      region.textContent = "";
      return;
    }
    if (latest === undefined || latest.role === userRole || latest.id <= mark.id) return;
    announced.current = { chat: props.chatKey, id: latest.id };
    region.textContent = latest.text;
  }, [props.chatKey, props.messages]);

  function selectChat(chatId: string): void {
    props.selectChat(chatId);
    if (globalThis.matchMedia?.("(max-width: 759px)").matches) setSidebarOpen(false);
  }

  function newChat(): void {
    props.newChat();
    if (globalThis.matchMedia?.("(max-width: 759px)").matches) setSidebarOpen(false);
  }

  function startSwipe(event: TouchEvent): void {
    touchStart.current = event.touches[0]?.clientX ?? 0;
  }

  function finishSwipe(event: TouchEvent): void {
    if (!globalThis.matchMedia?.("(max-width: 759px)").matches) return;
    const end = event.changedTouches[0]?.clientX ?? touchStart.current;
    const distance = end - touchStart.current;
    if (!sidebarOpen && touchStart.current < 28 && distance > 54) setSidebarOpen(true);
    if (sidebarOpen && distance < -54) setSidebarOpen(false);
  }

  return (
    <section
      className={`chat-shell ${sidebarOpen ? "sidebar-open" : "sidebar-closed"}`}
      onTouchStart={startSwipe}
      onTouchEnd={finishSwipe}
    >
      <button
        aria-label={t(strings, "action-close-history")}
        className="sidebar-scrim"
        type="button"
        onClick={() => setSidebarOpen(false)}
      />

      <aside className="sidebar">
        <div className="sidebar-brand">
          <span className="brand-mark" aria-hidden="true">
            A
          </span>
          <h1>{t(strings, "app-name")}</h1>
          <button
            aria-label={t(strings, sidebarOpen ? "action-collapse-sidebar" : "action-expand-sidebar")}
            aria-expanded={sidebarOpen}
            className="sidebar-toggle"
            type="button"
            onClick={() => setSidebarOpen(open => !open)}
          >
            <PanelIcon />
          </button>
        </div>

        <button className="new-chat" type="button" onClick={newChat}>
          <PlusIcon />
          <b>{t(strings, "action-new-chat")}</b>
        </button>

        <div className="search">
          <SearchIcon />
          <input
            aria-label={t(strings, "action-search-conversations")}
            autoComplete="off"
            name="conversation-search"
            placeholder={t(strings, "conversation-search-placeholder")}
            type="search"
            value={props.query}
            onChange={event => props.setQuery(event.target.value)}
          />
        </div>

        <nav className="history" aria-label={t(strings, "label-chat-history")}>
          <h2>{t(strings, "label-recent")}</h2>
          {props.history.length === 0 && (
            <span className="history-empty">
              {t(strings, props.query === "" ? "chat-history-empty" : "conversation-search-empty")}
            </span>
          )}
          {props.history.map(chat => (
            <button
              aria-current={props.chatId === chat.id ? "page" : undefined}
              className={props.chatId === chat.id ? "active" : ""}
              type="button"
              key={chat.id}
              title={chat.title}
              onClick={() => selectChat(chat.id)}
            >
              <span className="history-dot" aria-hidden="true" />
              <span className="history-title">{highlighted(chat)}</span>
              {chat.unread && (
                <span className="unread">
                  <span className="visually-hidden">{t(strings, "label-unread-messages")}</span>
                </span>
              )}
            </button>
          ))}
        </nav>

        <div className="sidebar-foot">
          <span aria-hidden="true" /> {t(strings, "chat-local-session")}
        </div>
      </aside>

      <main className="conversation">
        <header>
          <button
            aria-label={t(strings, "action-open-history")}
            className="mobile-menu"
            type="button"
            onClick={() => setSidebarOpen(true)}
          >
            <PanelIcon />
          </button>
          <div className="conversation-title">
            {/* The selected conversation's title is the core's answer; the
                label for one that does not exist yet is this surface's, because
                it is a localized string and nothing else. */}
            <h2>{props.title ?? t(strings, "action-new-conversation")}</h2>
            <span>{props.chatId ? t(strings, "chat-session-saved") : t(strings, "chat-draft-synced")}</span>
          </div>
          <div className={`status ${props.sending ? "busy" : ""}`} role="status">
            <span aria-hidden="true" />
            {props.sending ? t(strings, "chat-status-thinking") : t(strings, "availability-available")}
          </div>
        </header>

        <div className="messages">
          {props.isEmpty ? (
            <div className="empty-state">
              <span className="empty-mark" aria-hidden="true">
                A
              </span>
              <h3>{t(strings, "chat-empty-title")}</h3>
              <span>{t(strings, "chat-empty-hint")}</span>
            </div>
          ) : (
            // A log a reader can walk, but not one that reads itself out: the
            // region below announces the one message that just arrived, and two
            // live regions over the same text would say everything twice.
            <div className="message-list" role="log" aria-live="off" aria-label={t(strings, "label-transcript")}>
              {props.messages.map(message => (
                <Row key={message.id.toString()} message={message} />
              ))}
            </div>
          )}
        </div>
        <p className="visually-hidden" ref={announcer} aria-live="polite" aria-atomic="true" />

        <div className="composer-wrap">
          {props.error !== null && (
            <p className="error" role="alert">
              {props.error}
            </p>
          )}
          {/* React 19 runs a form action in a transition and suppresses the
              navigation itself, so there is no submit event to cancel. */}
          <form action={props.send}>
            <textarea
              aria-label={t(strings, "composer-placeholder")}
              autoComplete="off"
              name="message"
              ref={composer}
              rows={1}
              placeholder={t(strings, "composer-placeholder")}
              value={props.draft}
              onChange={event => props.setDraft(event.target.value)}
              onKeyDown={event => {
                if (event.key !== "Enter" || event.shiftKey) return;
                // A composition is still choosing characters: Enter commits it
                // and belongs to the input method, not to us.
                if (event.nativeEvent.isComposing) return;
                event.preventDefault();
                props.send();
              }}
            />
            {/* `canSend` is the node's answer -- `!is_sending && !cancelled`,
                never a check on the text -- so the button stays live until the
                request starts. An empty draft is refused in the handler. */}
            <button aria-label={t(strings, "action-send-message")} disabled={!props.canSend}>
              {props.sending ? spinner : <SendIcon />}
            </button>
          </form>
          <small>{t(strings, "composer-hint-multiline")}</small>
        </div>
      </main>
    </section>
  );
}

/** One message, with the time caption that opens its group. */
function Row({ message }: { message: ChatMessage }) {
  const strings = use(Strings);
  const mine = message.role === userRole;
  const at = message.startsTimeGroup ? acceptedAt(message.acceptedAtMs) : null;
  return (
    <Fragment>
      {at !== null && <p className="time-group">{stamp.format(at)}</p>}
      <article
        className={`message ${mine ? "user" : "assistant"}${message.endsSpeakerGroup ? " ends-group" : ""}`}
      >
        <span className="avatar" aria-hidden="true">
          {mine ? "Y" : "A"}
        </span>
        <div>
          {/* Hidden from the eye, kept for the ear: an outgoing message whose
              only speaker signal is its colour has none in greyscale. */}
          <span className={mine ? "speaker visually-hidden" : "speaker"}>
            {t(strings, mine ? "chat-role-you" : "chat-role-assistant")}
          </span>
          <p>{message.text}</p>
        </div>
      </article>
    </Fragment>
  );
}

/**
 * A title with the query's matches marked.
 *
 * The offsets are the core's, in Unicode scalars; a JavaScript string is
 * indexed in UTF-16 units, so this is the one conversion the surface owes.
 */
function highlighted(chat: ChatSummary): ReactNode {
  if (chat.matchRanges.length === 0) return chat.title;
  const points = [...chat.title];
  const parts: ReactNode[] = [];
  let at = 0;
  for (const range of chat.matchRanges as readonly MatchRange[]) {
    if (range.start > at) parts.push(points.slice(at, range.start).join(""));
    parts.push(<mark key={range.start}>{points.slice(range.start, range.end).join("")}</mark>);
    at = range.end;
  }
  if (at < points.length) parts.push(points.slice(at).join(""));
  return parts;
}

function PanelIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24">
      <path d="M4 5.5h16v13H4zM9 5.5v13" />
    </svg>
  );
}

function PlusIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24">
      <path d="M12 5v14M5 12h14" />
    </svg>
  );
}

function SearchIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24">
      <path d="M10.5 4a6.5 6.5 0 1 1 0 13 6.5 6.5 0 0 1 0-13Zm4.8 11.3L20 20" />
    </svg>
  );
}

function SendIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24">
      <path d="m5 12 14-7-4 14-3-6-7-1Zm7 1 7-8" />
    </svg>
  );
}
