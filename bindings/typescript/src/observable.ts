import type { ChatMessage, ChatState } from "@arut/ffi";

/** A projection read now, and re-read when its handle reports a change. */
export interface ObservableStore<T> {
  getSnapshot(): T;
  subscribe(listener: () => void): () => void;
  dispose(): void;
}

/** A revision stream: the generated `*Changes()` sessions, or a stand-in. */
export type Changes = AsyncIterable<bigint> & { dispose?(): void };

/**
 * A `useSyncExternalStore` source over one handle.
 *
 * The core reports a revision per fact, so a burst of them is coalesced into
 * one read on the next microtask: ten thousand revisions cost one read and one
 * render. Disposal stops the stream rather than waiting for the next revision.
 */
export function observe<T>(read: () => T, changes: () => Changes): ObservableStore<T> {
  const listeners = new Set<() => void>();
  const source = changes();
  const revisions = source[Symbol.asyncIterator]();
  let value = read();
  let disposed = false;
  let queued = false;
  const refresh = (): void => {
    if (disposed || queued) return;
    queued = true;
    queueMicrotask(() => {
      queued = false;
      if (disposed) return;
      value = read();
      for (const listener of [...listeners]) listener();
    });
  };
  void (async () => {
    while (!disposed) {
      const revision = await revisions.next();
      if (revision.done === true) return;
      refresh();
    }
  })().catch((error: unknown) => {
    if (!disposed) console.error(error);
  });
  return {
    getSnapshot: () => value,
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      listeners.clear();
      source.dispose?.();
      void revisions.return?.();
    },
  };
}

/** Cache immutable keyed rows while reading chat metadata on each revision. */
export function chatReader(chat: {
  state(): ChatState;
  messagesAfter(afterId: bigint): ChatMessage[];
}) {
  let messages: ChatMessage[] = [];
  return (): ChatState & { messages: ChatMessage[] } => {
    const state = chat.state();
    // A transcript that moved backwards is a different conversation: a handle
    // that rebound underneath us (the editor bridge does) has nothing to add to.
    if (state.lastMessageId < (messages.at(-1)?.id ?? 0n)) messages = [];
    const added = chat.messagesAfter(messages.at(-1)?.id ?? 0n);
    if (added.length) messages = messages.concat(added);
    return { ...state, messages };
  };
}

/** The native AbortSignal cancels both initialization and the stream follower. */
export function followComposer(composer: {
  initialize(options?: { signal?: AbortSignal }): Promise<unknown>;
  follow(options?: { signal?: AbortSignal }): Promise<unknown>;
}): () => void {
  const lifetime = new AbortController();
  const options = { signal: lifetime.signal };
  void composer
    .initialize(options)
    .then(() => composer.follow(options))
    .catch((error: unknown) => {
      if (!lifetime.signal.aborted) console.error(error);
    });
  return () => lifetime.abort();
}
