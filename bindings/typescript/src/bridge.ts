import type { Changes } from "./observable";

// A view that runs somewhere else -- a webview, with the session in its editor
// host (ADR 0011) -- reads projections the same way a view in this process does.
// What crosses is one whole projection per change and named intents back: no
// handle, and no sentence (ADR 0016). Nothing here knows what is projected.

/** One named projection a host publishes: what to read, and when it changes. */
export interface Published<T = unknown> {
  name: string;
  read: () => T;
  changes: () => Changes;
}

/** A projection on the wire: one entry per published name. */
export type Wire = Record<string, unknown>;

/** Host to view. */
export type HostMessage = { type: "projection"; values: Wire };

/** View to host. */
export type PortMessage = { type: "ready" } | { type: "intent"; name: string; payload?: unknown };

/** The published side: whatever holds the handles. */
export interface Host {
  /** The current projection, encoded as it crosses. */
  snapshot(): Wire;
  /** Post the current projection now -- the answer to `ready`. */
  publish(): void;
  /** Stop following every published stream. */
  dispose(): void;
}

/** The viewing side: whatever renders what crossed. */
export interface Port {
  /** The last projection received for `name`. */
  state<T>(name: string): T;
  /**
   * A revision stream that ticks whenever any of `names` changes.
   *
   * One reader often stands for several published names -- a draft a view
   * echoes has to be its own entry so the echo releases on the value itself,
   * while the state it belongs to keeps changing around it -- so a stream over
   * a set of them is what a reader asks for.
   */
  changes(...names: string[]): Changes;
  /** Ask the host to do something. */
  intent(name: string, payload?: unknown): void;
  /**
   * Hold a value of our own for `name` until the host confirms it.
   *
   * A field bound to a projection must not lag a keystroke; the local value
   * stands until the host's projection carries it back, which is the same
   * contract an in-process write keeps.
   */
  echo(name: string, value: unknown): void;
  /** Take one projection from the host. */
  receive(values: Wire): void;
}

/**
 * Follow every declared projection and post the whole set on any change.
 *
 * A burst of revisions -- one per fact the core revised -- is coalesced into one
 * post on the next microtask, across every declared entry: ten thousand
 * revisions cost one message. Reads happen at post time, so nothing is held here
 * that the handles do not already hold.
 */
export function projectionHost(
  published: readonly Published[],
  post: (message: HostMessage) => void,
): Host {
  let disposed = false;
  let queued = false;
  const snapshot = (): Wire => {
    const values: Wire = {};
    for (const entry of published) values[entry.name] = encode(entry.read());
    return values;
  };
  const publish = (): void => {
    if (!disposed) post({ type: "projection", values: snapshot() });
  };
  const streams = published.map(entry => entry.changes());
  const revised = streams.map(stream => stream[Symbol.asyncIterator]());
  for (const iterator of revised) {
    void (async () => {
      while (!disposed) {
        const revision = await iterator.next();
        if (revision.done === true) return;
        if (disposed || queued) continue;
        queued = true;
        queueMicrotask(() => {
          queued = false;
          publish();
        });
      }
    })().catch((error: unknown) => {
      if (!disposed) console.error(error);
    });
  }
  return {
    snapshot,
    publish,
    dispose() {
      if (disposed) return;
      disposed = true;
      for (const stream of streams) stream.dispose?.();
      for (const iterator of revised) void iterator.return?.();
    },
  };
}

/**
 * Read the last projection the host posted, and post intents back.
 *
 * `initial` is the projection the host embedded in the page, so the first render
 * reads a real projection rather than a fabricated one.
 */
export function projectionPort(initial: Wire, post: (message: PortMessage) => void): Port {
  const values = new Map<string, unknown>();
  const encoded = new Map<string, string>();
  const echoed = new Map<string, string>();
  const streams = new Map<string, ReturnType<typeof revisions>>();
  const ticking = (name: string) => {
    const found = streams.get(name);
    if (found) return found;
    const created = revisions();
    streams.set(name, created);
    return created;
  };
  const take = (name: string, value: unknown): void => {
    const text = JSON.stringify(value) ?? "";
    if (encoded.get(name) === text) return;
    encoded.set(name, text);
    values.set(name, decode(value));
    ticking(name).bump();
  };
  const port: Port = {
    state<T>(name: string): T {
      // The host declared what it publishes under this name; a port cannot
      // narrow what it was never told the shape of.
      return values.get(name) as T;
    },
    changes: (...names: string[]) => merged(names.map(ticking)),
    intent(name: string, payload?: unknown) {
      post({ type: "intent", name, payload: encode(payload) });
    },
    echo(name: string, value: unknown) {
      const wire = encode(value);
      echoed.set(name, JSON.stringify(wire) ?? "");
      take(name, wire);
    },
    receive(next: Wire) {
      for (const [name, value] of Object.entries(next)) {
        const pending = echoed.get(name);
        if (pending !== undefined) {
          // Ours until the host says it has it: an older projection would put
          // the field back the way it was before the person typed.
          if (pending !== (JSON.stringify(value) ?? "")) continue;
          echoed.delete(name);
        }
        take(name, value);
      }
    },
  };
  for (const [name, value] of Object.entries(initial)) take(name, value);
  post({ type: "ready" });
  return port;
}

/** An identifier on the wire: `structuredClone` carries `bigint`, JSON does not. */
interface Tagged {
  $u64: string;
}

function tagged(value: unknown): value is Tagged {
  return typeof value === "object" && value !== null && typeof (value as Partial<Tagged>).$u64 === "string";
}

/** Identifiers as text, everything else as itself. */
export function encode(value: unknown): unknown {
  if (typeof value === "bigint") return { $u64: value.toString() };
  if (Array.isArray(value)) return value.map(encode);
  if (typeof value === "object" && value !== null) {
    return Object.fromEntries(Object.entries(value).map(([key, entry]) => [key, encode(entry)]));
  }
  return value;
}

/** The same projection, with its identifiers back as `bigint`. */
export function decode(value: unknown): unknown {
  if (tagged(value)) return BigInt(value.$u64);
  if (Array.isArray(value)) return value.map(decode);
  if (typeof value === "object" && value !== null) {
    return Object.fromEntries(Object.entries(value).map(([key, entry]) => [key, decode(entry)]));
  }
  return value;
}

/** A revision counter every reader can await, in place of a generated stream. */
function revisions() {
  let revision = 0n;
  const listeners = new Set<(revision: bigint) => void>();
  return {
    bump() {
      revision += 1n;
      for (const listener of [...listeners]) listener(revision);
    },
    listen(listener: (revision: bigint) => void): () => void {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}

/** One stream over several counters: whichever ticks first wakes the reader. */
function merged(sources: readonly ReturnType<typeof revisions>[]): Changes {
  return {
    [Symbol.asyncIterator]: () => {
      let pending: ((result: IteratorResult<bigint>) => void) | null = null;
      // A tick nobody was awaiting is not lost: a reader that re-enters `next`
      // a microtask later still sees it.
      let missed: bigint | null = null;
      const wake = (revision: bigint) => {
        const resolve = pending;
        pending = null;
        if (resolve) resolve({ value: revision, done: false });
        else missed = revision;
      };
      const stop = sources.map(source => source.listen(wake));
      return {
        next: () => {
          if (missed !== null) {
            const revision = missed;
            missed = null;
            return Promise.resolve<IteratorResult<bigint>>({ value: revision, done: false });
          }
          return new Promise<IteratorResult<bigint>>(resolve => {
            pending = resolve;
          });
        },
        return: () => {
          for (const unlisten of stop) unlisten();
          pending = null;
          return Promise.resolve<IteratorResult<bigint>>({ value: undefined, done: true });
        },
      };
    },
  };
}
