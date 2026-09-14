import { useEffect, useState } from "react";
import type { ChatMessage, ChatSummary } from "@arut/bindings-typescript";
// Value imports name the module they come from: a webview renders a projection
// and must not pull the wasm core in behind a barrel (ADR 0011).
import { chatReader, followComposer, observe, type Changes } from "@arut/bindings-typescript/observable";
import { useObservable } from "@arut/bindings-typescript/react";
import { errorMessage } from "@arut/bindings-typescript/strings";
import type { ChatPort, ChatScope } from "./port";

/**
 * One handle's projection, re-read whenever that handle reports a change.
 *
 * `source` identifies the handle: a new one builds a new store and the old one
 * is disposed, so nothing here ever shows a fabricated state while a handle is
 * being replaced -- every render reads a real projection of a real handle.
 */
function useHandle<T>(source: object, read: () => T, changes: () => Changes): T {
  return useObservable(useOwned(source, () => observe(read, changes)));
}

/** A handle held for as long as `key` stands, and disposed when it does not. */
function useOwned<T extends { dispose(): void }>(key: unknown, open: () => T): T {
  const [held, setHeld] = useState(() => ({ key, value: open() }));
  let current = held;
  if (!Object.is(current.key, key)) {
    current.value.dispose();
    current = { key, value: open() };
    setHeld(current);
  }
  const value = current.value;
  useEffect(() => () => value.dispose(), [value]);
  return value;
}

/** Whatever the person is looking at, with the intents that change it. */
export function useChat(port: ChatPort) {
  const conversations = useOwned(port, () => port.conversations());
  const list = useHandle(
    conversations,
    (): { summaries: ChatSummary[]; selectedId: string | null } => ({
      summaries: conversations.state(),
      selectedId: conversations.selectedId(),
    }),
    () => conversations.listChanges(),
  );
  // The core owns which conversation is open, so every surface on this session
  // follows the same one. What it cannot name is a conversation that does not
  // exist yet, so asking for a fresh one is this surface's own state; a
  // selection made elsewhere replaces it when the list reports the change.
  const [choice, choose] = useState({ id: list.selectedId, fresh: 0 });
  const [seen, markSeen] = useState(list.selectedId);
  let showing = choice;
  if (!Object.is(list.selectedId, seen)) {
    markSeen(list.selectedId);
    if (!Object.is(list.selectedId, choice.id)) {
      showing = { id: list.selectedId, fresh: 0 };
      choose(showing);
    }
  }
  const chat = useOwned(`${showing.id ?? ""}#${showing.fresh}`, () => open(port, showing.id, showing.fresh));
  const composer = useOwned(chat, () => chat.composer());
  useEffect(() => followComposer(composer), [composer]);

  const transcript = useHandle(chat, read(chat), () => chat.chatChanges());
  const draft = useHandle(
    composer,
    () => ({ ...composer.state(), error: errorMessage(composer) }),
    () => composer.composerChanges(),
  );

  return {
    chatId: transcript.id,
    messages: transcript.messages,
    history: list.summaries,
    isEmpty: transcript.isEmpty,
    sending: transcript.isSending,
    canSend: transcript.canSend,
    draft: draft.text,
    error: transcript.error ?? draft.error,
    setDraft: (text: string) => void composer.replace(text),
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
  };
}

/** The conversation a selection names, or the pending one. */
function open(port: ChatPort, selectedId: string | null, fresh: number): ChatScope {
  if (selectedId !== null) return port.selectChat(selectedId) ?? port.chat();
  return fresh === 0 ? port.chat() : port.newChat();
}

/** The transcript, plus whatever sentence its error resolves to. */
function read(chat: ChatScope) {
  const transcript = chatReader(chat);
  return (): Omit<ReturnType<typeof transcript>, "error"> & {
    messages: ChatMessage[];
    error: string | null;
  } => ({ ...transcript(), error: errorMessage(chat) });
}
