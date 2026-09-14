import type {
  ChatHandle,
  ChatMessage,
  ChatRole,
  ChatState,
  ComposerHandle,
  ComposerState,
  ConversationsHandle,
  ErrorArg,
  ProductSessionHandle,
} from "@arut/bindings-typescript";
import { projectionPort, type PortMessage, type Wire } from "@arut/bindings-typescript/bridge";

// What the chat UI needs from a node, stated as the generated handles state it:
// every member below is `Pick`ed off the `.d.ts`, so a projection that gains a
// field or an error that changes shape reaches this file as a type error rather
// than as a second declaration to keep in step. The wasm session implements the
// port as itself; a relay route (ADR 0009) and the editor bridge implement it
// verbatim. Everything the UI reads is a projection plus a revision stream;
// everything it does is an intent that returns when the node has taken it.

export interface ChatPort {
  chat(): ChatScope;
  newChat(): ChatScope;
  selectChat(id: string): ChatScope | null;
  conversations(): ConversationsScope;
}

export interface ChatScope
  extends Pick<
    ChatHandle,
    "state" | "messagesAfter" | "send" | "errorKey" | "errorArgs" | "chatChanges" | "dispose"
  > {
  composer(): ComposerScope;
}

export type ComposerScope = Pick<
  ComposerHandle,
  "state" | "replace" | "initialize" | "follow" | "errorKey" | "errorArgs" | "composerChanges" | "dispose"
>;

export type ConversationsScope = Pick<
  ConversationsHandle,
  "state" | "selectedId" | "title" | "query" | "select" | "setQuery" | "listChanges" | "dispose"
>;

/**
 * `ChatRole.User`, restated. A view renders projections, so it must not import
 * the generated module for one number: in an editor webview the core lives in
 * the extension host (ADR 0011) and the wasm has no business in the page. The
 * generated `ChatRole` checks the value.
 */
export const userRole = 0 satisfies ChatRole;

/** wasm in the page: the generated session already is the port. */
export const wasmPort = (session: ProductSessionHandle): ChatPort => session;

/** A projection whose typed error travels as its Fluent id (ADR 0016). */
type Errored<T> = Omit<T, "error"> & { errorKey: string | null; errorArgs: ErrorArg[] };

/**
 * What the host publishes, one entry per name.
 *
 * The two fields a surface writes -- the draft and the search query -- are
 * entries of their own because a port holds its own value for an entry until
 * the host carries exactly that value back, and the state around them keeps
 * changing while the person is still typing.
 */
export interface ChatProjection {
  chat: Errored<ChatState>;
  messages: ChatMessage[];
  composer: Errored<Omit<ComposerState, "text">>;
  draft: string;
  conversations: ReturnType<ConversationsHandle["state"]>;
  selection: { selectedId: string | null; title: string | null };
  query: string;
}

/** Every name the host publishes, in the order a first projection is built. */
export const projected = [
  "chat",
  "messages",
  "composer",
  "draft",
  "conversations",
  "selection",
  "query",
] as const satisfies readonly (keyof ChatProjection)[];

/** What the webview asks the host to do. */
export type ChatIntent =
  | { name: "newChat" }
  | { name: "select"; id: string | null }
  | { name: "draft"; text: string }
  | { name: "send"; text: string }
  | { name: "query"; text: string };

/**
 * One intent off the bridge, or `null` for anything this surface does not name.
 *
 * The bridge carries `unknown`, because nothing in it knows what is projected;
 * this is the one place that decides what a payload had to be.
 */
export function chatIntent(name: string, payload: unknown): ChatIntent | null {
  switch (name) {
    case "newChat":
      return { name };
    case "select":
      return payload === null || typeof payload === "string" ? { name, id: payload } : null;
    case "draft":
    case "send":
    case "query":
      return typeof payload === "string" ? { name, text: payload } : null;
    default:
      return null;
  }
}

export interface BridgePort extends ChatPort {
  /** Take one projection from the host. */
  receive(values: Wire): void;
}

/**
 * A view over a node that lives somewhere else -- the VS Code extension host,
 * which keeps the session (ADR 0011) and posts whole projections.
 *
 * Reads answer from the last projection the generic port received; intents are
 * posted, and the draft and the query are held locally until the host carries
 * them back, so a control bound to one never lags a keystroke -- the same
 * contract `ComposerClient::replace` keeps in process.
 *
 * `initial` is the projection the host encoded into the page, so the first
 * render reads a real projection of a real session.
 */
export function bridgePort(post: (message: PortMessage) => void, initial: Wire): BridgePort {
  const port = projectionPort(initial, post);
  const read = <K extends keyof ChatProjection>(name: K): ChatProjection[K] =>
    port.state<ChatProjection[K]>(name);

  const chatState = (): ChatState => {
    const wire = read("chat");
    return {
      id: wire.id,
      lastMessageId: wire.lastMessageId,
      status: wire.status,
      canSend: wire.canSend,
      isSending: wire.isSending,
      isEmpty: wire.isEmpty,
      error: null,
    };
  };

  const composerState = (): ComposerState => {
    const wire = read("composer");
    return { text: read("draft"), revision: wire.revision, status: wire.status, error: null };
  };

  const composer = (): ComposerScope => ({
    state: composerState,
    replace: (text: string) => {
      port.echo("draft", text);
      port.intent("draft", text);
      return Promise.resolve(composerState());
    },
    // The host holds the subscription this port reports through.
    initialize: () => Promise.resolve(composerState()),
    follow: () => Promise.resolve(),
    errorKey: () => read("composer").errorKey,
    errorArgs: () => read("composer").errorArgs,
    composerChanges: () => port.changes("composer", "draft"),
    dispose: () => {},
  });

  const chat: ChatScope = {
    state: chatState,
    messagesAfter: (afterId: bigint) => read("messages").filter(message => message.id > afterId),
    composer,
    send: (text: string) => {
      port.intent("send", text);
      return Promise.resolve(chatState());
    },
    errorKey: () => read("chat").errorKey,
    errorArgs: () => read("chat").errorArgs,
    chatChanges: () => port.changes("chat", "messages"),
    dispose: () => {},
  };

  return {
    chat: () => chat,
    newChat: () => {
      port.intent("newChat");
      return chat;
    },
    selectChat: (id: string) => {
      port.intent("select", id);
      return chat;
    },
    conversations: () => ({
      state: () => read("conversations"),
      selectedId: () => read("selection").selectedId,
      title: () => read("selection").title,
      query: () => read("query"),
      select: (id: string | null) => port.intent("select", id),
      setQuery: (query: string) => {
        port.echo("query", query);
        port.intent("query", query);
      },
      listChanges: () => port.changes("conversations", "selection", "query"),
      dispose: () => {},
    }),
    receive: (values: Wire) => port.receive(values),
  };
}
