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

/**
 * Append-only keyed rows, read by cursor, with one rewind rule.
 *
 * Rows arrive through `after(lastId)`, so a revision costs the rows it added
 * rather than the whole list. A source that no longer holds the row this cursor
 * stands on has rebound -- the editor bridge points one handle at another
 * conversation -- and there is nothing to append to, so `read()` starts over.
 */
export function cursored<Row>(
  read: () => Row[],
  after: (afterId: bigint) => Row[],
  id: (row: Row) => bigint,
): () => Row[] {
  let rows: Row[] = [];
  return (): Row[] => {
    const last = rows.at(-1);
    if (last === undefined) return (rows = read());
    const mark = id(last);
    if (mark <= 0n) return (rows = read());
    // One row of overlap is the rewind test: the source still holds the row this
    // cursor stands on, or it is showing something else entirely.
    const tail = after(mark - 1n);
    const first = tail[0];
    if (first === undefined || id(first) !== mark) return (rows = read());
    if (tail.length > 1) rows = rows.concat(tail.slice(1));
    return rows;
  };
}

/** A handle whose projection is only kept current while something follows it. */
export interface Followed {
  initialize(options?: { signal?: AbortSignal }): Promise<unknown>;
  follow(options?: { signal?: AbortSignal }): Promise<unknown>;
}

/**
 * Runs a handle's initialize-then-follow lifetime, and returns the stop.
 *
 * The native `AbortSignal` cancels both halves, so disposal does not wait for
 * whatever the follow is currently awaiting.
 */
export function following(handle: Followed): () => void {
  const lifetime = new AbortController();
  const options = { signal: lifetime.signal };
  void handle
    .initialize(options)
    .then(() => handle.follow(options))
    .catch((error: unknown) => {
      if (!lifetime.signal.aborted) console.error(error);
    });
  return () => lifetime.abort();
}

/** Suppresses the write-back an edit of our own caused. */
export interface EchoGuard {
  readonly isApplying: boolean;
  apply<T>(body: () => T): T;
}

/**
 * A guard over the "our own echo" rule.
 *
 * A projection echoes what a surface just wrote -- a draft, a selection -- and
 * applying that echo back to the control the person is using would fight them.
 * Anything applied inside `apply` is ours, and the follower skips it.
 */
export function echoGuard(): EchoGuard {
  let depth = 0;
  return {
    get isApplying() {
      return depth > 0;
    },
    apply<T>(body: () => T): T {
      depth++;
      try {
        return body();
      } finally {
        depth--;
      }
    },
  };
}

/** The last instant a date can name, in epoch milliseconds: 9999-12-31T23:59:59.999Z. */
const LATEST_REPRESENTABLE_MS = 253_402_300_799_999n;

/**
 * An epoch-millisecond stamp as a date, or `null` when there is no such instant.
 *
 * Zero is "not stamped" and anything past the last representable date is a value
 * no clock produced; one bounds rule, so no surface writes the bound.
 */
export function acceptedAt(epochMilliseconds: bigint): Date | null {
  if (epochMilliseconds <= 0n || epochMilliseconds > LATEST_REPRESENTABLE_MS) return null;
  return new Date(Number(epochMilliseconds));
}
