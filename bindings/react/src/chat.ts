import { ChatStatus, type ChatStore } from "@arut/bindings-typescript";
import { useObservable } from "./observable";

export function useChat(store: ChatStore) {
  const snapshot = useObservable(store);

  return {
    draft: snapshot.draft,
    snapshot,
    sending: snapshot.status === ChatStatus.Sending,
    setDraft: (text: string) => store.setDraft(text),
    send: () => void store.send(),
    newChat: () => store.newChat(),
    history: store.history(),
    selectChat: (chatId: string) => store.selectChat(chatId),
  };
}
