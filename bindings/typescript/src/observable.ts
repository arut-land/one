import type { ChatHandle, ChatMessage, ComposerHandle } from "@arut/ffi";
import type { StreamCancellable } from "@boltffi/runtime";

export interface ObservableStore<T> {
  getSnapshot(): T;
  subscribe(listener: () => void): () => void;
  dispose(): void;
}

export class ObservableState<T> implements ObservableStore<T> {
  private readonly listeners = new Set<() => void>();
  private read: () => T;
  private stream: StreamCancellable<bigint>;
  private disposed = false;
  private refreshPending = false;
  private value: T;

  constructor(
    read: () => T,
    subscribe: (invalidate: () => void) => StreamCancellable<bigint>,
  ) {
    this.read = read;
    this.value = read();
    this.stream = subscribe(this.invalidate);
  }

  observe(
    read: () => T,
    subscribe: (invalidate: () => void) => StreamCancellable<bigint>,
  ): void {
    if (this.disposed) throw new Error("observable state is disposed");
    this.stream.cancel();
    this.read = read;
    this.value = read();
    this.stream = subscribe(this.invalidate);
    [...this.listeners].forEach((listener) => listener());
  }

  getSnapshot = (): T => this.value;

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.listeners.clear();
    this.stream.cancel();
  }

  private invalidate = (): void => {
    if (this.disposed || this.refreshPending) return;
    this.refreshPending = true;
    queueMicrotask(() => {
      this.refreshPending = false;
      if (this.disposed) return;
      this.value = this.read();
      [...this.listeners].forEach((listener) => listener());
    });
  };
}

export function observeScope<T>(handle: {
  state(): T;
  changes(listener: () => void): StreamCancellable<bigint>;
}): ObservableState<T> {
  return new ObservableState(() => handle.state(), listener => handle.changes(listener));
}

/** Cache immutable keyed rows while reading chat metadata on each invalidation. */
export function chatReader(chat: ChatHandle) {
  let messages: ChatMessage[] = [];
  return () => {
    const added = chat.messagesAfter(messages.at(-1)?.id ?? 0n);
    if (added.length) messages = messages.concat(added);
    return { ...chat.state(), messages };
  };
}

/** The native AbortSignal cancels both initialization and the stream follower. */
export function followComposer(composer: ComposerHandle): () => void {
  const lifetime = new AbortController();
  const options = { signal: lifetime.signal };
  void composer.initialize(options).then(() => composer.follow(options)).catch(error => {
    if (!lifetime.signal.aborted) console.error(error);
  });
  return () => lifetime.abort();
}
