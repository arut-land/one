import { createChatStore, useChat, type ChatStore } from "@arut/bindings-react";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { ChatView } from "./index";
import "./style.css";

const container = document.querySelector<HTMLElement>("#app");
if (!container) throw new Error("Missing app root");

function App({ store }: { store: ChatStore }) {
  return <ChatView {...useChat(store)} />;
}

const store = await createChatStore();
createRoot(container).render(
  <StrictMode>
    <App store={store} />
  </StrictMode>,
);
window.addEventListener("pagehide", () => store.dispose(), { once: true });
