import { ChatRole } from "@arut/bindings-react";
import type { useChat } from "../../../web/src/useChat";
import { useEffect, useRef, useState, type FormEvent, type TouchEvent } from "react";

type ChatPopupProps = ReturnType<typeof useChat>;

export function ChatPopup(props: ChatPopupProps) {
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const composer = useRef<HTMLInputElement>(null);
  const touchStart = useRef(0);
  const activeTitle = props.history.find((chat) => chat.id === props.snapshot.chatId)?.title;

  useEffect(() => {
    const close = (event: KeyboardEvent) => {
      if (event.key === "Escape") setSidebarOpen(false);
      if (event.key === "/" && document.activeElement !== composer.current) {
        event.preventDefault();
        composer.current?.focus();
      }
    };
    globalThis.addEventListener("keydown", close);
    return () => globalThis.removeEventListener("keydown", close);
  }, []);

  function submit(event: FormEvent): void {
    event.preventDefault();
    props.send();
  }

  function finishSwipe(event: TouchEvent): void {
    const end = event.changedTouches[0]?.clientX ?? touchStart.current;
    const distance = end - touchStart.current;
    if (!sidebarOpen && touchStart.current < 24 && distance > 48) setSidebarOpen(true);
    if (sidebarOpen && distance < -48) setSidebarOpen(false);
  }

  return (
    <section
      className={sidebarOpen ? "sidebar-open" : "sidebar-closed"}
      onTouchStart={(event) => { touchStart.current = event.touches[0]?.clientX ?? 0; }}
      onTouchEnd={finishSwipe}
    >
      <header>
        <button
          aria-label={sidebarOpen ? "Hide chat history" : "Show chat history"}
          accessKey="s"
          className="nav-toggle"
          type="button"
          onClick={() => setSidebarOpen((open) => !open)}
        ><PanelIcon /></button>
        <div className="title">
          <strong>{activeTitle ?? "New conversation"}</strong>
          <small>{props.sending ? "Arut is thinking" : "Local session"}</small>
        </div>
        <button className="new-chat" accessKey="n" type="button" onClick={props.newChat} title="New chat"><PlusIcon /></button>
      </header>

      <aside className="history" aria-label="Chat history">
        <div><b>ARUT</b><span>RECENT</span></div>
        {props.history.length === 0 && <p>No chats yet</p>}
        {props.history.map((chat) => (
          <button
            aria-current={props.snapshot.chatId === chat.id ? "page" : undefined}
            className={props.snapshot.chatId === chat.id ? "active" : ""}
            type="button"
            key={chat.id}
            title={chat.title}
            onClick={() => props.selectChat(chat.id)}
          >
            <i />{chat.title}
          </button>
        ))}
      </aside>

      <main className="messages" aria-live="polite">
        {props.snapshot.messages.length === 0 && (
          <div className="empty"><b>A</b><span>Start a conversation</span><small>Your draft stays in sync.</small></div>
        )}
        {props.snapshot.messages.map((message) => (
          <article className={message.role === ChatRole.User ? "user" : "assistant"} key={message.id.toString()}>
            <span>{message.role === ChatRole.User ? "You" : "Arut"}</span>
            <p>{message.text}</p>
          </article>
        ))}
      </main>

      <div className="composer">
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
            {props.sending ? <i /> : <SendIcon />}
          </button>
        </form>
      </div>
    </section>
  );
}

function PanelIcon() {
  return <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M4 5.5h16v13H4zM9 5.5v13" /></svg>;
}

function PlusIcon() {
  return <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" /></svg>;
}

function SendIcon() {
  return <svg aria-hidden="true" viewBox="0 0 24 24"><path d="m5 12 14-7-4 14-3-6-7-1Zm7 1 7-8" /></svg>;
}
