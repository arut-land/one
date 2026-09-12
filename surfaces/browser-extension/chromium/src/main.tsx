import { createChatStore, useChat, type ChatStore } from "@arut/bindings-react";
import { createRoot } from "react-dom/client";
import { ChatPopup } from "./index";
import "./style.css";

const container = document.querySelector<HTMLElement>("#app");
if (!container) throw new Error("Missing extension root");

function App({ store }: { store: ChatStore }) {
  return <ChatPopup {...useChat(store)} />;
}

const store = await createChatStore();
createRoot(container).render(<App store={store} />);
window.addEventListener("pagehide", () => store.dispose(), { once: true });
