import { useEffect, useMemo, useState } from "react";
import {
  ChatStatus,
  chatReader,
  followComposer,
  describeChatError,
  describeComposerError,
  observeScope,
  type ProductSessionHandle,
} from "@arut/bindings-typescript";
import { useObservable } from "@arut/bindings-typescript/react";

// Navigation is surface state. Every rendered projection has its own observer.
export function useChat(session: ProductSessionHandle) {
  const [chat, select] = useState(() => session.chat());
  const composer = useMemo(() => chat.composer(), [chat]);
  const transcript = useMemo(() => observeScope({ state: chatReader(chat), changes: cb => chat.chatChanges(cb) }), [chat]);
  const draft = useMemo(() => observeScope({ state: () => composer.state(), changes: cb => composer.composerChanges(cb) }), [composer]);
  const conversations = useMemo(() => session.conversations(), [session]);
  const list = useMemo(() => observeScope({ state: () => conversations.state(), changes: cb => conversations.listChanges(cb) }), [conversations]);
  useEffect(() => {
    const stop = followComposer(composer);
    return () => { stop(); transcript.dispose(); draft.dispose(); composer.dispose(); chat.dispose(); };
  }, [chat, composer, transcript, draft]);
  useEffect(() => () => { list.dispose(); conversations.dispose(); }, [list, conversations]);
  const state = useObservable(transcript);
  const composerState = useObservable(draft);
  const history = useObservable(list);
  // The chat's own error takes precedence; a composer-only failure (a
  // background resync, say) still needs to reach the person even when the
  // chat itself is idle.
  const error = state.error
    ? describeChatError(state.error)
    : composerState.error
      ? describeComposerError(composerState.error)
      : null;
  return {
    snapshot: { ...state, chatId: state.id, error },
    draft: composerState.text,
    history,
    sending: state.status === ChatStatus.Sending,
    setDraft: (text: string) => { void composer.replace(text); },
    send: () => { void chat.send(composer.state().text); },
    newChat: () => select(session.newChat()),
    selectChat: (id: string) => { const next = session.selectChat(id); if (next) select(next); },
  };
}
