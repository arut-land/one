import { ChatRole, type useChat } from "@arut/bindings-react";
import { useEffect, useRef, useState, type FormEvent, type TouchEvent } from "react";

type ChatViewProps = ReturnType<typeof useChat>;

export function ChatView(props: ChatViewProps) {
  const [sidebarOpen, setSidebarOpen] = useState(
    () => globalThis.matchMedia?.("(min-width: 760px)").matches ?? true,
  );
  const touchStart = useRef(0);
  const composer = useRef<HTMLInputElement>(null);
  const activeTitle = props.history.find((chat) => chat.id === props.snapshot.chatId)?.title;

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
        aria-label="Close chat history"
        className="sidebar-scrim"
        type="button"
        onClick={() => setSidebarOpen(false)}
      />

      <aside className="sidebar">
        <div className="sidebar-brand">
          <span className="brand-mark">A</span>
          <strong>Arut</strong>
          <button
            aria-label={sidebarOpen ? "Collapse sidebar" : "Expand sidebar"}
            accessKey="s"
            className="sidebar-toggle"
            type="button"
            onClick={() => setSidebarOpen((open) => !open)}
          >
            <PanelIcon />
          </button>
        </div>

        <button className="new-chat" accessKey="n" type="button" onClick={newChat}>
          <PlusIcon /><b>New chat</b>
        </button>

        <nav className="history" aria-label="Chat history">
          <p>Recent</p>
          {props.history.length === 0 && <span className="history-empty">Your chats will appear here.</span>}
          {props.history.map((chat) => (
            <button
              aria-current={props.snapshot.chatId === chat.id ? "page" : undefined}
              className={props.snapshot.chatId === chat.id ? "active" : ""}
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

        <div className="sidebar-foot"><span /> Local session</div>
      </aside>

      <main className="conversation">
        <header>
          <button
            aria-label="Open chat history"
            className="mobile-menu"
            type="button"
            onClick={() => setSidebarOpen(true)}
          ><PanelIcon /></button>
          <div className="conversation-title">
            <strong>{activeTitle ?? "New conversation"}</strong>
            <span>{props.snapshot.chatId ? "Saved in this session" : "Draft synced across this session"}</span>
          </div>
          <div className={`status ${props.sending ? "busy" : ""}`}>
            <span />{props.sending ? "Thinking" : "Ready"}
          </div>
        </header>

        <div className="messages" aria-live="polite">
          {props.snapshot.messages.length === 0 ? (
            <div className="empty-state">
              <span className="empty-mark">A</span>
              <p>What are we working on?</p>
              <span>Write a message below. Your draft stays with this conversation.</span>
            </div>
          ) : (
            <div className="message-list">
              {props.snapshot.messages.map((message) => (
                <article
                  className={message.role === ChatRole.User ? "message user" : "message assistant"}
                  key={message.id.toString()}
                >
                  <span className="avatar">{message.role === ChatRole.User ? "Y" : "A"}</span>
                  <div>
                    <span>{message.role === ChatRole.User ? "You" : "Arut"}</span>
                    <p>{message.text}</p>
                  </div>
                </article>
              ))}
            </div>
          )}
        </div>

        <div className="composer-wrap">
          {props.snapshot.error && <p className="error">{props.snapshot.error}</p>}
          <form onSubmit={submit}>
            <input
              aria-label="Message Arut"
              autoFocus
              ref={composer}
              placeholder="Message Arut"
              value={props.draft}
              onChange={(event) => props.setDraft(event.target.value)}
            />
            <button aria-label="Send message" disabled={props.sending || !props.draft.trim()}>
              {props.sending ? <span className="spinner" /> : <SendIcon />}
            </button>
          </form>
          <small>Enter to send</small>
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
  return <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" /></svg>;
}

function SendIcon() {
  return <svg aria-hidden="true" viewBox="0 0 24 24"><path d="m5 12 14-7-4 14-3-6-7-1Zm7 1 7-8" /></svg>;
}
