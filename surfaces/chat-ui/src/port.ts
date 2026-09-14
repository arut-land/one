import type {
  Changes,
  ChatMessage,
  ChatRole,
  ChatState,
  ChatSummary,
  ComposerState,
  ProductSessionHandle,
} from "@arut/bindings-typescript";

// What the chat UI needs from a node, in the shape of the generated session
// handle rather than a shape of the view's own: the wasm session implements it
// as itself, and a relay route (ADR 0009) implements it verbatim. Everything
// the UI reads is a projection plus a revision stream; everything it does is an
// intent that returns when the node has taken it.

export interface ChatPort {
  chat(): ChatScope;
  newChat(): ChatScope;
  selectChat(id: string): ChatScope | null;
  conversations(): ConversationsScope;
}

export interface ChatScope {
  state(): ChatState;
  messagesAfter(afterId: bigint): ChatMessage[];
  composer(): ComposerScope;
  send(text: string): Promise<unknown>;
  /** The Fluent id of the current error, and its arguments positionally. */
  errorKey(): string | null;
  errorArgs(): string[];
  chatChanges(): Changes;
  dispose(): void;
}

export interface ComposerScope {
  state(): ComposerState;
  replace(text: string): Promise<unknown>;
  initialize(options?: { signal?: AbortSignal }): Promise<unknown>;
  follow(options?: { signal?: AbortSignal }): Promise<unknown>;
  errorKey(): string | null;
  errorArgs(): string[];
  composerChanges(): Changes;
  dispose(): void;
}

export interface ConversationsScope {
  state(): ChatSummary[];
  selectedId(): string | null;
  select(id: string | null): void;
  listChanges(): Changes;
  dispose(): void;
}

/**
 * `ChatRole.User`, restated. A view renders projections, so it must not import
 * the generated module for one number: in an editor webview the core lives in
 * the extension host (ADR 0011) and the wasm has no business in the page. The
 * generated `ChatRole` checks the value.
 */
export const userRole = 0 satisfies ChatRole;

/** wasm in the page: the generated session already is the port. */
export const wasmPort = (session: ProductSessionHandle): ChatPort => session;

/**
 * One whole projection, as the VS Code extension host posts it.
 *
 * Identifiers are decimal text because `structuredClone` carries `bigint` but
 * the webview's message channel is not guaranteed to, and no sentence crosses:
 * the error travels as its Fluent id (ADR 0016), which the webview formats
 * from the same catalog the host would have used.
 */
export interface ChatProjection {
  chat: Sent<ChatState, "lastMessageId">;
  messages: Text<ChatMessage, "id" | "acceptedAtMs">[];
  composer: Sent<ComposerState, "revision">;
  conversations: ChatSummary[];
  selectedId: string | null;
}

/** A projection on the wire: identifiers as decimal text. */
type Text<T, Ids extends keyof T> = Omit<T, Ids> & Record<Ids, string>;

/** The same, for a state whose typed error travels as its message key. */
type Sent<T extends { error: unknown }, Ids extends keyof T> =
  Text<Omit<T, "error">, Exclude<Ids, "error">> & { errorKey: string | null; errorArgs: string[] };

/** What the webview asks the host to do. */
export type ChatCommand =
  /** The page is mounted: post a projection, in case one was missed. */
  | { type: "ready" }
  | { type: "newChat" }
  | { type: "select"; id: string | null }
  | { type: "draft"; text: string }
  | { type: "send"; text: string };

export interface BridgePort extends ChatPort {
  /** Take one projection from the host. */
  receive(projection: ChatProjection): void;
}

/**
 * A view over a node that lives somewhere else -- the VS Code extension host,
 * which keeps the session (ADR 0011) and posts whole projections.
 *
 * Reads answer from the last projection; intents are posted and echoed locally
 * so a text field bound to `ComposerState.text` never lags a keystroke, the
 * same contract `ComposerClient::replace` keeps in process.
 */
export function bridgePort(post: (command: ChatCommand) => void, initial: ChatProjection): BridgePort {
  let projection = initial;
  let echoed: string | null = null;
  const chatChanges = new Revisions();
  const composerChanges = new Revisions();
  const listChanges = new Revisions();

  const composer = (): ComposerScope => ({
    state: () => ({
      ...projection.composer,
      text: echoed ?? projection.composer.text,
      revision: BigInt(projection.composer.revision),
      error: null,
    }),
    replace: (text: string) => {
      echoed = text;
      composerChanges.bump();
      post({ type: "draft", text });
      return Promise.resolve();
    },
    // The host holds the subscription this port reports through.
    initialize: () => Promise.resolve(),
    follow: () => Promise.resolve(),
    errorKey: () => projection.composer.errorKey,
    errorArgs: () => projection.composer.errorArgs,
    composerChanges: () => composerChanges.stream(),
    dispose: () => {},
  });

  const chat = (): ChatScope => ({
    state: () => ({ ...projection.chat, lastMessageId: BigInt(projection.chat.lastMessageId), error: null }),
    messagesAfter: (afterId: bigint) =>
      projection.messages
        .filter(message => BigInt(message.id) > afterId)
        .map(message => ({ ...message, id: BigInt(message.id), acceptedAtMs: BigInt(message.acceptedAtMs) })),
    composer,
    send: (text: string) => {
      post({ type: "send", text });
      return Promise.resolve();
    },
    errorKey: () => projection.chat.errorKey,
    errorArgs: () => projection.chat.errorArgs,
    chatChanges: () => chatChanges.stream(),
    dispose: () => {},
  });

  return {
    chat,
    newChat: () => {
      post({ type: "newChat" });
      return chat();
    },
    selectChat: (id: string) => {
      post({ type: "select", id });
      return chat();
    },
    conversations: () => ({
      state: () => projection.conversations,
      selectedId: () => projection.selectedId,
      select: (id: string | null) => {
        // Echoed the way a draft is, so a read right after a write is true
        // before the host has answered.
        projection = { ...projection, selectedId: id };
        listChanges.bump();
        post({ type: "select", id });
      },
      listChanges: () => listChanges.stream(),
      dispose: () => {},
    }),
    receive(next: ChatProjection) {
      projection = next;
      if (echoed === next.composer.text) echoed = null;
      chatChanges.bump();
      composerChanges.bump();
      listChanges.bump();
    },
  };
}

/** A revision counter every reader can await, in place of a wasm stream. */
class Revisions {
  private revision = 0n;
  private readonly waiting = new Set<(revision: bigint) => void>();

  bump(): void {
    this.revision += 1n;
    const woken = [...this.waiting];
    this.waiting.clear();
    for (const wake of woken) wake(this.revision);
  }

  stream(): Changes {
    const waiting = this.waiting;
    return {
      [Symbol.asyncIterator]: () => ({
        next: () =>
          new Promise<IteratorResult<bigint>>(resolve => {
            waiting.add(revision => resolve({ value: revision, done: false }));
          }),
        return: () => Promise.resolve({ value: undefined, done: true }),
      }),
    };
  }
}
