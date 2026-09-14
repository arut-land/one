import { createContext, use, useEffect, useRef, useState, type FormEvent, type TouchEvent } from "react";
import { t, type L10nBundle } from "@arut/bindings-typescript/strings";
import { userRole } from "./port";
import type { useChat } from "./useChat";

/** The negotiated strings, supplied once by the composition root. */
export const Strings = createContext<L10nBundle>({ format: (key: string) => key });

type ChatViewProps = ReturnType<typeof useChat>;

const spinner = <span className="spinner" />;

export function ChatView(props: ChatViewProps) {
  const strings = use(Strings);
  const [sidebarOpen, setSidebarOpen] = useState(
    () => globalThis.matchMedia?.("(min-width: 760px)").matches ?? true,
  );
  const touchStart = useRef(0);
  const composer = useRef<HTMLInputElement>(null);
  const activeTitle = props.history.find(chat => chat.id === props.chatId)?.title;

  useEffect(() => {
    const desktop = globalThis.matchMedia?.("(min-width: 760px)");
    const adapt = (event: MediaQueryListEvent) => setSidebarOpen(event.matches);
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setSidebarOpen(false);
      if (event.key === "/" && document.activeElement !== composer.current) {
        event.preventDefault();
        composer.current?.focus();
      }
    };
    desktop?.addEventListener("change", adapt);
    globalThis.addEventListener("keydown", closeOnEscape);
    return () => {
      desktop?.removeEventListener("change", adapt);
      globalThis.removeEventListener("keydown", closeOnEscape);
    };
  }, []);

  function submit(event: FormEvent): void {
    event.preventDefault();
    props.send();
  }

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
          <span className="brand-mark">A</span>
          <strong>{t(strings, "app-name")}</strong>
          <button
            aria-label={t(strings, sidebarOpen ? "action-collapse-sidebar" : "action-expand-sidebar")}
            aria-expanded={sidebarOpen}
            accessKey="s"
            className="sidebar-toggle"
            type="button"
            onClick={() => setSidebarOpen(open => !open)}
          >
            <PanelIcon />
          </button>
        </div>

        <button className="new-chat" accessKey="n" type="button" onClick={newChat}>
          <PlusIcon />
          <b>{t(strings, "action-new-chat")}</b>
        </button>

        <nav className="history" aria-label={t(strings, "label-chat-history")}>
          <p>{t(strings, "label-recent")}</p>
          {props.history.length === 0 && (
            <span className="history-empty">{t(strings, "chat-history-empty")}</span>
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
              <span className="history-dot" />
              <span>{chat.title}</span>
            </button>
          ))}
        </nav>

        <div className="sidebar-foot">
          <span /> {t(strings, "chat-local-session")}
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
            <strong>{activeTitle ?? t(strings, "action-new-conversation")}</strong>
            <span>{props.chatId ? t(strings, "chat-session-saved") : t(strings, "chat-draft-synced")}</span>
          </div>
          <div className={`status ${props.sending ? "busy" : ""}`}>
            <span />
            {props.sending ? t(strings, "chat-status-thinking") : t(strings, "availability-available")}
          </div>
        </header>

        <div className="messages" aria-label={t(strings, "label-transcript")} aria-live="polite">
          {props.isEmpty ? (
            <div className="empty-state">
              <span className="empty-mark">A</span>
              <p>{t(strings, "chat-empty-title")}</p>
              <span>{t(strings, "chat-empty-hint")}</span>
            </div>
          ) : (
            <div className="message-list">
              {props.messages.map(message => (
                <article
                  className={message.role === userRole ? "message user" : "message assistant"}
                  key={message.id.toString()}
                >
                  <span className="avatar">{message.role === userRole ? "Y" : "A"}</span>
                  <div>
                    <span>
                      {message.role === userRole
                        ? t(strings, "chat-role-you")
                        : t(strings, "chat-role-assistant")}
                    </span>
                    <p>{message.text}</p>
                  </div>
                </article>
              ))}
            </div>
          )}
        </div>

        <div className="composer-wrap">
          {props.error !== null && <p className="error">{props.error}</p>}
          <form onSubmit={submit}>
            <input
              aria-label={t(strings, "composer-placeholder")}
              autoComplete="off"
              autoFocus
              name="message"
              ref={composer}
              placeholder={t(strings, "composer-placeholder")}
              value={props.draft}
              onChange={event => props.setDraft(event.target.value)}
              onKeyDown={event => {
                if (event.key === "Enter" && event.nativeEvent.isComposing) event.preventDefault();
              }}
            />
            <button aria-label={t(strings, "action-send-message")} disabled={!props.canSend}>
              {props.sending ? spinner : <SendIcon />}
            </button>
          </form>
          <small>{t(strings, "composer-hint-enter")}</small>
        </div>
      </main>
    </section>
  );
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

function SendIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24">
      <path d="m5 12 14-7-4 14-3-6-7-1Zm7 1 7-8" />
    </svg>
  );
}
