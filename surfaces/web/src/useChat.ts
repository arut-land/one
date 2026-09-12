import { useEffect, useMemo, useState } from "react";
import {
  ChatStatus,
  chatReader,
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
  const list = useMemo(() => observeScope({ state: () => session.conversations().state(), changes: cb => session.conversations().listChanges(cb) }), [session]);
  useEffect(() => { void composer.initialize(); void composer.follow(); return () => { transcript.dispose(); draft.dispose(); }; }, [composer, transcript, draft]);
  useEffect(() => () => list.dispose(), [list]);
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
