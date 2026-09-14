import { createRoot, type Root } from "react-dom/client";
import type { L10nBundle } from "@arut/bindings-typescript/strings";
import { ChatView, Strings } from "./ChatView";
import { useChat } from "./useChat";
import type { ChatPort } from "./port";
import "./style.css";

export { ChatView, Strings } from "./ChatView";
export { useChat } from "./useChat";
export * from "./port";

/** Render the chat over one port, with the strings its host negotiated. */
export function mountChat(element: HTMLElement, port: ChatPort, strings: L10nBundle): Root {
  const root = createRoot(element);
  root.render(
    <Strings value={strings}>
      <Chat port={port} />
    </Strings>,
  );
  return root;
}

function Chat({ port }: { port: ChatPort }) {
  return <ChatView {...useChat(port)} />;
}
