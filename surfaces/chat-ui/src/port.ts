import type {
  ChatHandle,
  ChatRole,
  ComposerHandle,
  ConversationsHandle,
  ProductSessionHandle,
} from "@arut/bindings-typescript";

// What the chat UI needs from a node, stated as the generated handles state it:
// every member below is `Pick`ed off the `.d.ts`, so a projection that gains a
// field or an error that changes shape reaches this file as a type error rather
// than as a second declaration to keep in step. The wasm session implements the
// port as itself. Everything the UI reads is a projection plus a revision
// stream; everything it does is an intent that returns when the node has taken
// it.

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
  | "state"
  | "selectedId"
  | "title"
  | "query"
  | "select"
  | "setQuery"
  | "rename"
  | "delete"
  | "listChanges"
  | "dispose"
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
