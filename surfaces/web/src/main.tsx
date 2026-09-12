import * as ffi from "@arut/ffi";
import { createRoot } from "react-dom/client";
import { ChatView } from "./index";
import { useChat } from "./useChat";
import "./style.css";
await (ffi as typeof ffi & { initialized: Promise<void> }).initialized;
const session = ffi.createProductSession("local-demo");
function App() { return <ChatView {...useChat(session)} />; }
createRoot(document.querySelector<HTMLElement>("#app")!).render(<App />);
window.addEventListener("pagehide", () => session.dispose(), { once: true });
