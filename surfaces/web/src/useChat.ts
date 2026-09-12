import { useEffect, useState } from "react";
import {
  ChatStatus,
  ComposerStatus,
  chatReader,
  followComposer,
  describeChatError,
  describeComposerError,
  observeScope,
  type ChatHandle,
  type ChatMessage,
  type ChatState,
  type ChatSummary,
  type ComposerState,
  type ObservableStore,
  type ProductSessionHandle,
} from "@arut/bindings-typescript";
import { useObservable } from "@arut/bindings-typescript/react";

function constantStore<T>(value: T): ObservableStore<T> {
  return { getSnapshot: () => value, subscribe: () => () => {}, dispose: () => {} };
}
const emptyTranscript = constantStore<ChatState & { messages: ChatMessage[] }>({
  id: null, lastMessageId: 0n, status: ChatStatus.Idle, error: null, messages: [],
});
const emptyDraft = constantStore<ComposerState>({
  text: "", revision: 0n, status: ComposerStatus.Connecting, error: null,
});
const emptyList = constantStore<ChatSummary[]>([]);

function bindChat(chat: ChatHandle) {
  const composer = chat.composer();
  const transcript = observeScope({ state: chatReader(chat), changes: cb => chat.chatChanges(cb) });
  const draft = observeScope({ state: () => composer.state(), changes: cb => composer.composerChanges(cb) });
  const stop = followComposer(composer);
  return {
    chat, composer, transcript, draft,
    dispose() {
      stop();
      transcript.dispose();
      draft.dispose();
      composer.dispose();
      chat.dispose();
    },
  };
}

type Selection = { kind: "current" | "new" } | { kind: "existing"; id: string };

export function useChat(session: ProductSessionHandle) {
  const [selection, select] = useState<Selection>({ kind: "current" });
  const [binding, setBinding] = useState<ReturnType<typeof bindChat> | null>(null);
  const [list, setList] = useState<ObservableStore<ChatSummary[]>>(emptyList);

  // Each effect setup owns fresh handles, including Strict Mode's replay.
  useEffect(() => {
    const chat = selection.kind === "new" ? session.newChat()
      : selection.kind === "existing" ? session.selectChat(selection.id) ?? session.chat()
      : session.chat();
    const next = bindChat(chat);
    setBinding(next);
    return () => next.dispose();
  }, [session, selection]);
  useEffect(() => {
    const conversations = session.conversations();
    const next = observeScope({ state: () => conversations.state(), changes: cb => conversations.listChanges(cb) });
    setList(next);
    return () => { next.dispose(); conversations.dispose(); };
  }, [session]);

  const state = useObservable(binding?.transcript ?? emptyTranscript);
  const composerState = useObservable(binding?.draft ?? emptyDraft);
  const history = useObservable(list);
  const error = state.error ? describeChatError(state.error)
    : composerState.error ? describeComposerError(composerState.error) : null;
  return {
    snapshot: { ...state, chatId: state.id, error },
    draft: composerState.text,
    history,
    sending: !binding || state.status === ChatStatus.Sending,
    setDraft: (text: string) => { void binding?.composer.replace(text); },
    send: () => { if (binding) void binding.chat.send(binding.composer.state().text); },
    newChat: () => select({ kind: "new" }),
    selectChat: (id: string) => select({ kind: "existing", id }),
  };
}
