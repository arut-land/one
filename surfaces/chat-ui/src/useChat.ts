import { useEffect, useRef, useState } from "react";
import type { ChatMessage } from "@arut/bindings-typescript";
// Value imports name the module they come from: a webview renders a projection
// and must not pull the wasm core in behind a barrel (ADR 0011).
import { cursored, following } from "@arut/bindings-typescript/observable";
import { useHandle, useOwned } from "@arut/bindings-typescript/react";
import { errorMessage } from "@arut/bindings-typescript/strings";
import type { ChatPort, ChatScope } from "./port";

/** Whatever the person is looking at, with the intents that change it. */
export function useChat(port: ChatPort) {
  const conversations = useOwned(port, () => port.conversations());
  const list = useHandle(
    conversations,
    () => ({
      // Every one of these is the core's answer, not this surface's: the search
      // predicate, the window title, the unread rule and the match offsets are
      // decided once in Rust and rendered by all five surfaces (ADR 0021).
      summaries: conversations.state(),
      selectedId: conversations.selectedId(),
      title: conversations.title(),
      query: conversations.query(),
    }),
    () => conversations.listChanges(),
  );
  // The core owns which conversation is open, so every surface on this session
  // follows the same one. What it cannot name is a conversation that does not
  // exist yet, so asking for a fresh one is this surface's own state; a
  // selection made elsewhere replaces it when the list reports the change.
  const [choice, choose] = useState({ id: list.selectedId, fresh: 0 });
  const [seen, markSeen] = useState(list.selectedId);
  // The id the pending conversation on screen gained when it was established.
  // The core selects it at that moment; the handle already open is that
  // conversation, so it is kept rather than replaced mid-send.
  const established = useRef<string | null>(null);
  let showing = choice;
  if (!Object.is(list.selectedId, seen)) {
    markSeen(list.selectedId);
    const adopted = choice.id === null && list.selectedId !== null && established.current === list.selectedId;
    if (!adopted && !Object.is(list.selectedId, choice.id)) {
      showing = { id: list.selectedId, fresh: 0 };
      choose(showing);
    }
  }
  // Which conversation is on screen, across the moment a pending one is saved
  // and gains an id of its own: that is one conversation, not two.
  const chatKey = `${showing.id ?? ""}#${showing.fresh}`;
  const chat = useOwned(chatKey, () => open(port, showing.id, showing.fresh));
  const composer = useOwned(chat, () => chat.composer());
  useEffect(() => following(composer), [composer]);

  const transcript = useHandle(chat, read(chat), () => chat.chatChanges());
  established.current = transcript.id;
  const draft = useHandle(
    composer,
    () => ({ ...composer.state(), error: errorMessage(composer) }),
    () => composer.composerChanges(),
  );

  return {
    chatId: transcript.id,
    chatKey,
    messages: transcript.messages,
    history: list.summaries,
    title: list.title,
    query: list.query,
    isEmpty: transcript.isEmpty,
    sending: transcript.isSending,
    canSend: transcript.canSend,
    draft: draft.text,
    error: transcript.error ?? draft.error,
    setDraft: (text: string) => void composer.replace(text),
    setQuery: (text: string) => conversations.setQuery(text),
    send: () => {
      if (draft.text.trim() !== "") void chat.send(draft.text);
    },
    newChat: () => {
      conversations.select(null);
      choose(current => ({ id: null, fresh: current.fresh + 1 }));
    },
    selectChat: (id: string) => {
      conversations.select(id);
      choose({ id, fresh: 0 });
    },
    renameChat: (id: string, title: string) => conversations.rename(id, title),
    deleteChat: (id: string) => conversations.delete(id),
  };
}

/** The conversation a selection names, or the pending one. */
function open(port: ChatPort, selectedId: string | null, fresh: number): ChatScope {
  if (selectedId !== null) return port.selectChat(selectedId) ?? port.chat();
  return fresh === 0 ? port.chat() : port.newChat();
}

/** The transcript, plus whatever sentence its error resolves to. */
function read(chat: ChatScope) {
  const messages = cursored<ChatMessage>(
    () => chat.messagesAfter(0n),
    afterId => chat.messagesAfter(afterId),
    message => message.id,
  );
  return () => ({ ...chat.state(), messages: messages(), error: errorMessage(chat) });
}
