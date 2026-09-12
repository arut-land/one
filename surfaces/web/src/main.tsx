import { createSession } from "@arut/runtime-browser";
import { createRoot } from "react-dom/client";
import { ChatView } from "./index";
import { useChat } from "./useChat";
import "./style.css";
const session = await createSession("local-demo");
function App() { return <ChatView {...useChat(session)} />; }
createRoot(document.querySelector<HTMLElement>("#app")!).render(<App />);
window.addEventListener("pagehide", () => session.dispose(), { once: true });
