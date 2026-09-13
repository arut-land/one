import { uuidV7 } from "../../../runtimes/browser/ids";
import { createSession, loadStrings } from "@arut/bindings-typescript";
import { createRoot } from "react-dom/client";
import { ChatView } from "./index";
import { useChat } from "./useChat";
import "./style.css";
// Strings come from the generated Fluent copy under public/locales (ADR 0022),
// negotiated against navigator.languages before the first render so no sentence
// is ever rendered as its message id.
await loadStrings({ baseUrl: "/locales", preferred: navigator.languages });
const session = await createSession("local-demo", { newId: uuidV7 });
function App() { return <ChatView {...useChat(session)} />; }
createRoot(document.querySelector<HTMLElement>("#app")!).render(<App />);
window.addEventListener("pagehide", () => session.dispose(), { once: true });
